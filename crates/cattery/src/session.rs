//! One session in the daemon, as a window shows it: created and attached
//! on a connection, its screen arriving as events and its input leaving
//! through a queue.

use std::path::Path;

use anyhow::Context as _;

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

/// Create a session running `shell`, or the user's login shell when it
/// names no command, in `cwd`, and attach `conn` to it.
///
/// Without `cwd`, the session starts where Cattery was started from. Its
/// environment is Cattery's either way, as a terminal opened from where
/// Cattery was would have it.
pub async fn create(
    conn: &mut Connection,
    shell: &ShellConfig,
    cwd: Option<&Path>,
    socket: &Path,
) -> anyhow::Result<SessionInfo> {
    let mut args = launch_args(Vec::new(), shell, &Carrier::Local(Endpoint::from(socket)))?;
    if let Some(cwd) = cwd {
        args.cwd = cwd
            .to_str()
            .context("the directory to open in is not UTF-8")?
            .to_owned();
    }
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
