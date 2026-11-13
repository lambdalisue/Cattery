//! Cattery's daemon as the `cattery` executable starts it: a process of its
//! own, apart from the window that connected to it.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cattery::daemon;
use cattery::session::{self, Event, Link};
use felis_client_core::config::ShellConfig;
use felis_client_core::{Connection, Offer, ShadowScreen};
use felis_protocol::messages::StopMode;

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
async fn attached(daemon: &Daemon, command: &[&str]) -> (mpsc::Receiver<Event>, Link) {
    attached_in(daemon, None, command).await
}

async fn attached_in(
    daemon: &Daemon,
    cwd: Option<&Path>,
    command: &[&str],
) -> (mpsc::Receiver<Event>, Link) {
    let mut conn = daemon.connect().await;
    let shell = ShellConfig {
        command: command.iter().map(|s| (*s).to_owned()).collect(),
    };
    let info = session::create(&mut conn, &shell, cwd, &daemon.socket)
        .await
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let link = session::pump(conn, info.id, move |event| tx.send(event).is_ok());
    (rx, link)
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

#[tokio::test(flavor = "multi_thread")]
async fn a_session_finds_the_terminfo_entry_its_term_names() {
    let daemon = Daemon::new();
    // The entries the test runs with, as an installed felis leaves, are left
    // out: only the folder the daemon puts first counts, and it must be new.
    let inherited = std::env::var("TERMINFO_DIRS").unwrap_or_default();
    let script = format!(
        "first=\"${{TERMINFO_DIRS%%:*}}\"; \
         [ \"$first\" != \"{}\" ] && \
         TERMINFO_DIRS=\"$first\" HOME=/nonexistent TERMINFO= infocmp \"$TERM\" >/dev/null \
         && echo found; sleep 10",
        inherited.split(':').next().unwrap_or_default(),
    );
    let (events, _outgoing) = attached(&daemon, &["/bin/sh", "-c", &script]).await;
    let mut shadow = ShadowScreen::new(24, 80);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(Event::Grid(msg)) = events.recv_timeout(Duration::from_millis(100)) {
            shadow.apply(&msg).unwrap();
            if top_row(&shadow) == "found" {
                return;
            }
        }
    }
    panic!("the entry was not found: {:?}", top_row(&shadow));
}

/// Wait until `shadow` shows `text` on its top row.
fn wait_for(events: &mpsc::Receiver<Event>, shadow: &mut ShadowScreen, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(Event::Grid(msg)) = events.recv_timeout(Duration::from_millis(100)) {
            shadow.apply(&msg).unwrap();
            if top_row(shadow) == text {
                return;
            }
        }
    }
    panic!("the screen never showed {text:?}: {:?}", top_row(shadow));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_outlives_the_window_it_was_shown_in() {
    let daemon = Daemon::new();
    let (events, link) = attached(&daemon, &["/bin/sh", "-c", "printf kept; sleep 30"]).await;
    let mut shadow = ShadowScreen::new(24, 80);
    wait_for(&events, &mut shadow, "kept");
    let id = link.detach().await;

    let listed = session::list(&daemon.socket).await.unwrap();
    let kept = listed.iter().find(|info| info.id == id).unwrap();
    assert!(kept.attachments.is_empty() && !kept.exited);

    let mut conn = daemon.connect().await;
    session::attach(&mut conn, id).await.unwrap();
    let (tx, again) = mpsc::channel();
    let _link = session::pump(conn, id, move |event| tx.send(event).is_ok());
    let mut shadow = ShadowScreen::new(24, 80);
    wait_for(&again, &mut shadow, "kept");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ended_session_leaves_the_list() {
    let daemon = Daemon::new();
    let (_events, link) = attached(&daemon, &["/bin/sh", "-c", "sleep 30"]).await;
    let id = link.detach().await;
    session::end(&daemon.socket, id).await.unwrap();
    let listed = session::list(&daemon.socket).await.unwrap();
    assert!(listed.iter().all(|info| info.id != id));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_window_is_told_of_an_image_a_program_draws() {
    let daemon = Daemon::new();
    // A one-pixel black RGB image, by the kitty graphics protocol.
    let (events, _link) = attached(
        &daemon,
        &[
            "/bin/sh",
            "-c",
            "printf '\\033_Ga=T,f=24,s=1,v=1;AAAA\\033\\\\'; sleep 10",
        ],
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(Event::Image(_)) = events.recv_timeout(Duration::from_millis(100)) {
            return;
        }
    }
    panic!("no image was told of");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_session_s_history_is_searched() {
    let daemon = Daemon::new();
    let (events, link) = attached(
        &daemon,
        &[
            "/bin/sh",
            "-c",
            "printf 'alpha\\nbeta\\nalpha\\n'; sleep 10",
        ],
    )
    .await;
    let mut shadow = ShadowScreen::new(24, 80);
    wait_for(&events, &mut shadow, "alpha");
    // The output lands a moment after its first line.
    std::thread::sleep(Duration::from_millis(300));
    let stream = link.search("alpha").unwrap();
    let mut found = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Found(id, _)) if id == stream => found += 1,
            Ok(Event::SearchEnded(id, ended)) if id == stream => {
                assert_eq!(ended, Ok(2));
                assert_eq!(found, 2);
                return;
            }
            _ => {}
        }
    }
    panic!("the search never ended; {found} found");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_notification_a_program_posts_is_relayed() {
    let daemon = Daemon::new();
    drop(daemon.connect().await);
    let (tx, posted) = mpsc::channel();
    let socket = daemon.socket.clone();
    let watching = tokio::spawn(async move {
        cattery::notify::watch(&socket, move |each| tx.send(each).is_ok()).await;
    });
    // Posted once the watch has had time to subscribe.
    let (_events, link) = attached(
        &daemon,
        &[
            "/bin/sh",
            "-c",
            "sleep 1; printf '\\033]777;notify;Build;done\\033\\\\'; sleep 10",
        ],
    )
    .await;
    let posted = tokio::task::spawn_blocking(move || posted.recv_timeout(Duration::from_secs(10)))
        .await
        .unwrap()
        .expect("no notification was relayed");
    watching.abort();
    assert_eq!(posted.session, link.id);
    assert_eq!(
        (posted.title.as_str(), posted.body.as_str()),
        ("Build", "done")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_daemon_takes_its_sessions_and_a_new_one_starts_empty() {
    let daemon = Daemon::new();
    let (_events, _link) = attached(&daemon, &["/bin/sh", "-c", "sleep 30"]).await;
    session::stop_daemon(&daemon.socket).await.unwrap();
    drop(daemon.connect().await);
    assert!(session::list(&daemon.socket).await.unwrap().is_empty());
}
