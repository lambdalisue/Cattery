//! One session in the daemon, as a window shows it: created and attached
//! on a connection, its screen arriving as events and its input leaving
//! through a queue.

use std::path::Path;
use std::time::Duration;

use anyhow::Context as _;

use felis_client_core::config::ShellConfig;
use felis_client_core::{AttachIntent, Carrier, Connection, Offer, OutgoingFrame, launch_args};
use felis_protocol::MessageKind;
use felis_protocol::messages::{GridMsg, PushMsg, SessionInfo, SessionToDaemonMsg};
use felis_transport::{ClientDriver, Delivery, Endpoint, Incoming, OwnedFrame};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task::JoinHandle;

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

/// Attach `conn` to the session `id`, whose screen then arrives whole.
pub async fn attach(conn: &mut Connection, id: u128) -> anyhow::Result<SessionInfo> {
    // Deliberate: the user picked it, so one whose program has ended is
    // attached all the same, to read what it left.
    Ok(conn.attach(id, AttachIntent::Deliberate).await?)
}

/// The sessions the daemon at `socket` keeps.
pub async fn list(socket: &Path) -> anyhow::Result<Vec<SessionInfo>> {
    let mut ops = felis_client_core::connect(socket, Offer::ops()).await?;
    Ok(ops.list_sessions().await?)
}

/// End the session `id` in the daemon at `socket`, and its program with it.
pub async fn end(socket: &Path, id: u128) -> anyhow::Result<()> {
    let mut ops = felis_client_core::connect(socket, Offer::ops()).await?;
    ops.destroy_session(format!("{id:032x}")).await?;
    Ok(())
}

/// A window's way to its session: the queue to it, and the task writing
/// what is queued.
pub struct Link {
    pub id: u128,
    outgoing: UnboundedSender<OutgoingFrame>,
    writer: JoinHandle<()>,
}

impl Link {
    pub fn send(&self, frame: OutgoingFrame) {
        // A closed queue means the connection ended, which the reader
        // reports on its own.
        _ = self.outgoing.send(frame);
    }

    /// Leave the session running without this window, once what is queued
    /// has gone out; the session's id.
    pub async fn detach(self) -> u128 {
        if let Ok(frame) = OutgoingFrame::ordered(&SessionToDaemonMsg::Detach) {
            self.send(frame);
        }
        drop(self.outgoing);
        // A daemon that has stopped reading cannot keep the window waiting.
        _ = tokio::time::timeout(Duration::from_secs(5), self.writer).await;
        self.id
    }
}

/// Start reading `conn`, attached to the session `id`, into `notify`, and
/// writing what is sent on the returned link. Reading stops when `notify`
/// returns false; writing stops once the link is detached or dropped and
/// the queue is drained.
pub fn pump(conn: Connection, id: u128, notify: impl Fn(Event) -> bool + Send + 'static) -> Link {
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
    let writing = tokio::spawn(async move {
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
    Link {
        id,
        outgoing,
        writer: writing,
    }
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
