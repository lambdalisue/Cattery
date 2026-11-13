//! Cattery's daemon as the `cattery` executable starts it: a process of its
//! own, apart from the window that connected to it.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cattery::daemon;
use cattery::session::{self, Event};
use felis_client_core::config::ShellConfig;
use felis_client_core::{Connection, Offer, OutgoingFrame, ShadowScreen};
use felis_protocol::messages::StopMode;
use tokio::sync::mpsc::UnboundedSender;

const CATTERY: &str = env!("CARGO_BIN_EXE_cattery");

/// A daemon of the test's own, stopped when the test ends however it ends.
struct Daemon {
    socket: PathBuf,
    _dir: tempfile::TempDir,
}

impl Daemon {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // The daemon makes the directory itself, private to the user.
        let socket = dir.path().join("cattery").join("daemon.sock");
        Self { socket, _dir: dir }
    }

    async fn connect(&self) -> Connection {
        daemon::connect(&self.socket, Path::new(CATTERY))
            .await
            .unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let socket = self.socket.clone();
        // A test's runtime may be the one panicking, so stop from a fresh one.
        let stopped = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                if let Ok(mut ops) = felis_client_core::connect(&socket, Offer::ops()).await {
                    _ = ops.daemon_stop(StopMode::Force).await;
                }
            });
        });
        _ = stopped.join();
    }
}

/// The session's events, and the queue to it, which holds the connection
/// open while it lives.
async fn attached(
    daemon: &Daemon,
    command: &[&str],
) -> (mpsc::Receiver<Event>, UnboundedSender<OutgoingFrame>) {
    attached_in(daemon, None, command).await
}

async fn attached_in(
    daemon: &Daemon,
    cwd: Option<&Path>,
    command: &[&str],
) -> (mpsc::Receiver<Event>, UnboundedSender<OutgoingFrame>) {
    let mut conn = daemon.connect().await;
    let shell = ShellConfig {
        command: command.iter().map(|s| (*s).to_owned()).collect(),
    };
    session::create(&mut conn, &shell, cwd, &daemon.socket)
        .await
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let outgoing = session::pump(conn, move |event| tx.send(event).is_ok());
    (rx, outgoing)
}

fn top_row(shadow: &ShadowScreen) -> String {
    let screen = shadow.screen();
    felis_grid::row_text_trim(screen.row_cells(0).unwrap(), screen.cluster_table())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daemon_is_started_when_none_answers() {
    let daemon = Daemon::new();
    daemon.connect().await;
    assert!(daemon.socket.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daemon_already_answering_is_connected_to_again() {
    let daemon = Daemon::new();
    daemon.connect().await;
    // Nothing could start a second daemon here.
    daemon::connect(&daemon.socket, Path::new("/no/such/cattery"))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn what_the_program_prints_reaches_the_window_s_screen() {
    let daemon = Daemon::new();
    let (events, _outgoing) = attached(&daemon, &["/bin/sh", "-c", "printf hello; sleep 10"]).await;
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
    let daemon = Daemon::new();
    let (events, _outgoing) = attached(&daemon, &["/bin/sh", "-c", "exit 0"]).await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(Event::Exited) = events.recv_timeout(Duration::from_millis(100)) {
            return;
        }
    }
    panic!("no exit was reported");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_starts_in_the_directory_it_is_opened_in() {
    let daemon = Daemon::new();
    let place = tempfile::tempdir().unwrap();
    let place = place.path().canonicalize().unwrap();
    let (events, _outgoing) =
        attached_in(&daemon, Some(&place), &["/bin/sh", "-c", "pwd; sleep 10"]).await;
    let mut shadow = ShadowScreen::new(24, 80);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(Event::Grid(msg)) = events.recv_timeout(Duration::from_millis(100)) {
            shadow.apply(&msg).unwrap();
            if top_row(&shadow) == place.display().to_string() {
                return;
            }
        }
    }
    panic!("the session started elsewhere: {:?}", top_row(&shadow));
}
