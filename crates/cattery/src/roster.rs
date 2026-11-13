//! The sessions Cattery's daemon keeps, as the session selector lists them.

use std::path::Path;
use std::time::{Duration, SystemTime};

use felis_protocol::messages::{ProgramStatusRecord, SessionInfo};

/// What a session is doing, as far as its windows go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Shown in this many windows.
    Shown(usize),
    /// Shown in none, for this long.
    Background(Duration),
    /// Its program has ended; the daemon lets it go shortly.
    Ended,
}

/// A command line the shell reported, as it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub line: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Succeeded,
    Failed,
}

/// One session, as a row of the selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: u128,
    pub title: String,
    /// Where it is, with the home directory as `~`.
    pub place: String,
    pub state: State,
    /// The program in front, as the system names it.
    pub process: Option<String>,
    /// Where it was started, with the home directory as `~`.
    pub started_in: Option<String>,
    /// When Cattery created it; `None` for one it did not, or created
    /// before it noted when.
    pub created: Option<SystemTime>,
    /// How the last command the shell marked ended.
    pub exit: Option<u32>,
    /// The last command line the shell reported, and whether it still runs.
    pub command: Option<Ran>,
    /// Its size, in columns and rows.
    pub size: (u16, u16),
}

/// The rows for `sessions`, newest first, with `home` written as `~`.
pub fn rows(sessions: &[SessionInfo], home: Option<&Path>) -> Vec<Row> {
    let mut sessions: Vec<&SessionInfo> = sessions.iter().collect();
    sessions.sort_by_key(|session| std::cmp::Reverse(session.sequence));
    sessions
        .into_iter()
        .map(|session| Row {
            id: session.id,
            title: session
                .title
                .clone()
                .filter(|title| !title.is_empty())
                .or_else(|| session.foreground.clone())
                .unwrap_or_else(|| "Shell".to_owned()),
            place: place(session, home),
            process: session.foreground.clone(),
            started_in: session
                .spawn_cwd
                .as_deref()
                .map(|dir| crate::title::shortened(dir, home)),
            created: session.tags.iter().find_map(|tag| created(tag)),
            exit: session.last_exit_code,
            command: session.program_status.iter().find_map(ran),
            size: (session.dims.cols, session.dims.rows),
            state: if session.exited {
                State::Ended
            } else if session.attachments.is_empty() {
                State::Background(Duration::from_secs(session.idle_seconds.unwrap_or(0)))
            } else {
                State::Shown(session.attachments.len())
            },
        })
        .collect()
}

fn place(session: &SessionInfo, home: Option<&Path>) -> String {
    let reported = session
        .cwd
        .as_deref()
        .map(felis_client_core::pipe::local_cwd_from_osc7)
        .filter(|dir| !dir.is_empty());
    let Some(dir) = reported.or_else(|| session.spawn_cwd.clone()) else {
        return String::new();
    };
    crate::title::shortened(&dir, home)
}

/// The command line Cattery's shell integration reported in `record`.
fn ran(record: &ProgramStatusRecord) -> Option<Ran> {
    if record.id != "shell" || record.app.as_deref() != Some("cattery") {
        return None;
    }
    let outcome = match record.state.as_str() {
        "working" => Outcome::Running,
        "done" => Outcome::Succeeded,
        "error" => Outcome::Failed,
        _ => return None,
    };
    Some(Ran {
        line: record.title.clone().filter(|line| !line.is_empty())?,
        outcome,
    })
}

/// The tag Cattery gives a session it creates at `now`, saying when.
pub fn created_tag(now: SystemTime) -> String {
    let seconds = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{CREATED}{seconds}")
}

const CREATED: &str = "cattery-created=";

/// When the tag `tag` says its session was created, where it says so.
fn created(tag: &str) -> Option<SystemTime> {
    let seconds = tag.strip_prefix(CREATED)?.parse().ok()?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
}

/// How long ago `then` was, at `now`, in words.
pub fn ago(then: SystemTime, now: SystemTime) -> String {
    let elapsed = now.duration_since(then).unwrap_or_default().as_secs();
    match elapsed {
        0..60 => "Just now".to_owned(),
        60..3600 => format!("{} min ago", elapsed / 60),
        3600..86_400 => format!("{} h ago", elapsed / 3600),
        _ => format!("{} days ago", elapsed / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use felis_protocol::messages::{Attachment, GridDims};

    use super::*;

    fn session(id: u128, sequence: u64) -> SessionInfo {
        SessionInfo {
            id,
            dims: GridDims {
                rows: 24,
                cols: 80,
                pixel_w: 0,
                pixel_h: 0,
            },
            title: None,
            cwd: None,
            idle_seconds: Some(0),
            tags: Vec::new(),
            last_notification: None,
            foreground: None,
            exited: false,
            last_exit_code: None,
            attachments: Vec::new(),
            sequence: NonZeroU64::new(sequence).unwrap(),
            program_status: Vec::new(),
            spawn_cwd: None,
            terminal_progress: None,
        }
    }

    fn attachment(id: u64) -> Attachment {
        Attachment {
            id,
            attached_at: SystemTime::UNIX_EPOCH,
            input_owner: false,
            carrier: None,
        }
    }

    #[test]
    fn a_session_tells_what_runs_in_it_where_it_started_and_how_it_went() {
        let mut busy = session(1, 1);
        busy.foreground = Some("vim".to_owned());
        busy.spawn_cwd = Some("/home/me/src".to_owned());
        busy.last_exit_code = Some(2);
        let row = &rows(&[busy], Some(Path::new("/home/me")))[0];
        assert_eq!(row.process.as_deref(), Some("vim"));
        assert_eq!(row.started_in.as_deref(), Some("~/src"));
        assert_eq!(row.exit, Some(2));
        assert_eq!(row.size, (80, 24));
    }

    #[test]
    fn a_session_tells_the_last_command_its_shell_reported() {
        let record = |state: &str, app: Option<&str>| ProgramStatusRecord {
            id: "shell".to_owned(),
            state: state.to_owned(),
            kind: None,
            progress: None,
            app: app.map(str::to_owned),
            title: Some("cargo test".to_owned()),
            msg: None,
        };
        let mut running = session(1, 1);
        running.program_status = vec![record("working", Some("cattery"))];
        let mut failed = session(2, 2);
        failed.program_status = vec![record("error", Some("cattery"))];
        let mut foreign = session(3, 3);
        foreign.program_status = vec![record("done", Some("claude"))];
        let rows = rows(&[running, failed, foreign], None);
        let ran = |line: &str, outcome| {
            Some(Ran {
                line: line.to_owned(),
                outcome,
            })
        };
        assert_eq!(rows[2].command, ran("cargo test", Outcome::Running));
        assert_eq!(rows[1].command, ran("cargo test", Outcome::Failed));
        assert_eq!(rows[0].command, None);
    }

    #[test]
    fn a_session_cattery_created_tells_when() {
        let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let mut tagged = session(1, 1);
        tagged.tags = vec!["other".to_owned(), created_tag(then)];
        let rows = rows(&[tagged, session(2, 2)], None);
        assert_eq!(rows[1].created, Some(then));
        assert_eq!(rows[0].created, None);
    }

    #[test]
    fn how_long_ago_is_told_in_words() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let before = |seconds| now - Duration::from_secs(seconds);
        assert_eq!(ago(before(10), now), "Just now");
        assert_eq!(ago(before(5 * 60), now), "5 min ago");
        assert_eq!(ago(before(3 * 3600), now), "3 h ago");
        assert_eq!(ago(before(2 * 86_400), now), "2 days ago");
        assert_eq!(ago(now + Duration::from_secs(5), now), "Just now");
    }

    #[test]
    fn the_newest_session_comes_first() {
        let ids: Vec<u128> = rows(&[session(1, 1), session(3, 3), session(2, 2)], None)
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(ids, [3, 2, 1]);
    }

    #[test]
    fn a_session_is_called_what_its_program_calls_it_or_what_runs_in_it() {
        let mut titled = session(1, 1);
        titled.title = Some("vim notes.md".to_owned());
        titled.foreground = Some("vim".to_owned());
        let mut untitled = session(2, 2);
        untitled.foreground = Some("zsh".to_owned());
        let rows = rows(&[titled, untitled, session(3, 3)], None);
        assert_eq!(rows[2].title, "vim notes.md");
        assert_eq!(rows[1].title, "zsh");
        assert_eq!(rows[0].title, "Shell");
    }

    #[test]
    fn a_session_is_where_its_shell_said_or_else_where_it_started() {
        let dir = tempfile::tempdir().unwrap();
        let mut reported = session(1, 1);
        reported.cwd = Some(format!("file://{}", dir.path().display()));
        reported.spawn_cwd = Some("/elsewhere".to_owned());
        let mut started = session(2, 2);
        started.spawn_cwd = Some("/srv/www".to_owned());
        let rows = rows(&[reported, started], None);
        assert_eq!(rows[1].place, dir.path().display().to_string());
        assert_eq!(rows[0].place, "/srv/www");
    }

    #[test]
    fn the_home_directory_is_written_as_a_tilde() {
        let mut inside = session(1, 1);
        inside.spawn_cwd = Some("/home/me/src/cattery".to_owned());
        let mut home = session(2, 2);
        home.spawn_cwd = Some("/home/me".to_owned());
        let mut beside = session(3, 3);
        beside.spawn_cwd = Some("/home/meg".to_owned());
        let rows = rows(&[inside, home, beside], Some(Path::new("/home/me")));
        assert_eq!(rows[2].place, "~/src/cattery");
        assert_eq!(rows[1].place, "~");
        assert_eq!(rows[0].place, "/home/meg");
    }

    #[test]
    fn a_session_is_shown_in_the_background_or_ended() {
        let mut shown = session(1, 1);
        shown.idle_seconds = None;
        shown.attachments = vec![attachment(1), attachment(2)];
        let mut background = session(2, 2);
        background.idle_seconds = Some(90);
        let mut ended = session(3, 3);
        ended.exited = true;
        let rows = rows(&[shown, background, ended], None);
        assert_eq!(rows[2].state, State::Shown(2));
        assert_eq!(rows[1].state, State::Background(Duration::from_secs(90)));
        assert_eq!(rows[0].state, State::Ended);
    }
}
