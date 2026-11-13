//! One session in the daemon, as a window shows it: created and attached
//! on a connection, its screen arriving as events and its input leaving
//! through a queue.

use std::path::Path;

use felis_client_core::config::ShellConfig;
use felis_client_core::{Carrier, Connection, OutgoingFrame, launch_args};
use felis_protocol::MessageKind;
use felis_protocol::messages::{GridMsg, PushMsg, SessionInfo};
use felis_transport::{ClientDriver, Delivery, Endpoint, Incoming, OwnedFrame};
use tokio::sync::mpsc::{self, UnboundedSender};

/// What the daemon tells a window about its session.
#[derive(Debug)]
pub enum Event {
    /// A change to the session's screen.
    Grid(GridMsg),
    /// The program in the session ended.
    Exited,
    /// The connection to the daemon ended.
    Closed,
}

/// Create a session running `command`, or the user's shell when it is
/// empty, and attach `conn` to it.
pub async fn create(
    conn: &mut Connection,
    command: Vec<String>,
    socket: &Path,
) -> anyhow::Result<SessionInfo> {
    // The working directory and environment are the launcher's, as a
    // terminal opened from it would have them.
    let args = launch_args(
        command,
        &ShellConfig::default(),
        &Carrier::Local(Endpoint::from(socket)),
    )?;
    Ok(conn.create_with(args).await?)
}

/// Start reading `conn` into `notify` and writing what is queued on the
/// returned sender. Reading stops when `notify` returns false; writing
/// stops once every sender is dropped and the queue is drained.
pub fn pump(
    conn: Connection,
    notify: impl Fn(Event) -> bool + Send + 'static,
) -> UnboundedSender<OutgoingFrame> {
    let Connection {
        mut reader,
        mut writer,
        mut driver,
        ..
    } = conn;
    tokio::spawn(async move {
        loop {
            let frame = match reader.next_frame().await {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(err) => {
                    tracing::warn!(?err, "reading from the daemon failed");
                    break;
                }
            };
            match route(&mut driver, &frame) {
                Route::Skip => {}
                Route::Stop => break,
                Route::Event(event) => {
                    if !notify(event) {
                        return;
                    }
                }
            }
        }
        notify(Event::Closed);
    });
    let (outgoing, mut queued) = mpsc::unbounded_channel::<OutgoingFrame>();
    tokio::spawn(async move {
        while let Some(out) = queued.recv().await {
            if let Err(err) = writer.send_checked_unflushed(out.frame()).await {
                tracing::warn!(?err, "writing to the daemon failed");
                break;
            }
            if let Err(err) = writer.flush().await {
                tracing::warn!(?err, "writing to the daemon failed");
                break;
            }
        }
    });
    outgoing
}

enum Route {
    Event(Event),
    Skip,
    Stop,
}

fn route(driver: &mut ClientDriver, frame: &OwnedFrame) -> Route {
    let payload = match driver.classify(frame) {
        Ok(Incoming::Payload(payload)) => payload,
        Ok(Incoming::CancelIgnored { .. }) => return Route::Skip,
        // A window opens no stream and makes no request after attaching,
        // so any other control message is one it cannot follow.
        Ok(Incoming::Control(msg)) => {
            tracing::warn!(?msg, "unexpected control message from the daemon");
            return Route::Stop;
        }
        Err(err) => {
            tracing::warn!(?err, "a frame from the daemon was refused");
            return Route::Stop;
        }
    };
    match payload.kind {
        MessageKind::Grid => match driver.decode::<GridMsg>(&payload) {
            Ok(Delivery::Deliver(delivered)) => Route::Event(Event::Grid(delivered.msg)),
            Ok(_) => Route::Skip,
            Err(err) => {
                tracing::warn!(?err, "a screen update from the daemon was refused");
                Route::Stop
            }
        },
        MessageKind::Push => match driver.decode::<PushMsg>(&payload) {
            Ok(Delivery::Deliver(delivered)) => match delivered.msg {
                PushMsg::SessionExited { .. } => Route::Event(Event::Exited),
                _ => Route::Skip,
            },
            Ok(_) => Route::Skip,
            Err(err) => {
                tracing::warn!(?err, "a notice from the daemon was refused");
                Route::Stop
            }
        },
        // Images, regions and the rest are not shown yet.
        _ => Route::Skip,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use felis_client_core::ShadowScreen;

    use super::*;
    use crate::daemon::testing;

    /// The session's events, and the queue to it, which holds the
    /// connection open while it lives.
    async fn attached(
        dir: &tempfile::TempDir,
        command: &[&str],
    ) -> (mpsc::Receiver<Event>, UnboundedSender<OutgoingFrame>) {
        let (mut conn, socket) = testing::connected(dir).await;
        let command = command.iter().map(|s| (*s).to_owned()).collect();
        create(&mut conn, command, &socket).await.unwrap();
        let (tx, rx) = mpsc::channel();
        let outgoing = pump(conn, move |event| tx.send(event).is_ok());
        (rx, outgoing)
    }

    fn top_row(shadow: &ShadowScreen) -> String {
        let screen = shadow.screen();
        felis_grid::row_text_trim(screen.row_cells(0).unwrap(), screen.cluster_table())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn what_the_program_prints_reaches_the_window_s_screen() {
        let dir = tempfile::tempdir().unwrap();
        let (events, _outgoing) =
            attached(&dir, &["/bin/sh", "-c", "printf hello; sleep 10"]).await;
        let mut shadow = ShadowScreen::new(24, 80);
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(Event::Grid(msg)) = events.recv_timeout(Duration::from_millis(100)) {
                shadow.apply(&msg).unwrap();
                if top_row(&shadow).starts_with("hello") {
                    return;
                }
            }
        }
        panic!("the screen never showed the output: {:?}", top_row(&shadow));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_window_is_told_when_the_program_ends() {
        let dir = tempfile::tempdir().unwrap();
        let (events, _outgoing) = attached(&dir, &["/bin/sh", "-c", "exit 0"]).await;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(Event::Exited) = events.recv_timeout(Duration::from_millis(100)) {
                return;
            }
        }
        panic!("no exit was reported");
    }
}
