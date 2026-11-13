//! The sessions Cattery's daemon keeps, as the session selector lists them.

use std::path::Path;
use std::time::Duration;

use felis_protocol::messages::SessionInfo;

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

/// One session, as a row of the selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: u128,
    pub title: String,
    /// Where it is, with the home directory as `~`.
    pub place: String,
    pub state: State,
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
    match home.and_then(|home| Path::new(&dir).strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir,
    }
}

/// The rows matching `query`, best first: rows whose title or place holds the
/// query's letters in order, ignoring case, those holding them unbroken
/// before those holding them apart. An empty query matches every row, in
/// the order given.
pub fn filtered<'a>(rows: &'a [Row], query: &str) -> Vec<&'a Row> {
    let query = query.to_lowercase();
    let mut found: Vec<(usize, usize, &Row)> = rows
        .iter()
        .enumerate()
        .filter_map(|(at, row)| {
            let score = [&row.title, &row.place]
                .into_iter()
                .filter_map(|text| distance(&text.to_lowercase(), &query))
                .min()?;
            Some((score, at, row))
        })
        .collect();
    found.sort_by_key(|(score, at, _)| (*score, *at));
    found.into_iter().map(|(_, _, row)| row).collect()
}

/// How far apart `query`'s letters lie in `text`, in order: none for an
/// unbroken run; `None` where they are not all there in order.
fn distance(text: &str, query: &str) -> Option<usize> {
    if text.contains(query) {
        return Some(0);
    }
    let mut letters = text.chars().enumerate();
    let mut last = None;
    let mut apart = 0;
    for wanted in query.chars() {
        let (at, _) = letters.find(|(_, letter)| *letter == wanted)?;
        if let Some(last) = last {
            apart += at - last - 1;
        }
        last = Some(at);
    }
    // Apart by nothing still is not unbroken: the run itself was not there.
    Some(apart + 1)
}

/// `state` in words.
pub fn describe(state: State) -> String {
    match state {
        State::Shown(1) => "Open in a window".to_owned(),
        State::Shown(windows) => format!("Open in {windows} windows"),
        State::Background(idle) if idle < Duration::from_secs(60) => "In the background".to_owned(),
        State::Background(idle) if idle < Duration::from_secs(3600) => {
            format!("In the background for {} min", idle.as_secs() / 60)
        }
        State::Background(idle) => format!("In the background for {} h", idle.as_secs() / 3600),
        State::Ended => "Ended".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::time::SystemTime;

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

    fn row(id: u128, title: &str, place: &str) -> Row {
        Row {
            id,
            title: title.to_owned(),
            place: place.to_owned(),
            state: State::Shown(1),
        }
    }

    fn ids(rows: &[&Row]) -> Vec<u128> {
        rows.iter().map(|row| row.id).collect()
    }

    #[test]
    fn nothing_typed_lists_every_session() {
        let rows = [row(1, "vim", "~"), row(2, "zsh", "~/src")];
        assert_eq!(ids(&filtered(&rows, "")), [1, 2]);
    }

    #[test]
    fn what_is_typed_is_found_in_the_title_or_the_place() {
        let rows = [row(1, "vim notes.md", "~"), row(2, "zsh", "~/src/cattery")];
        assert_eq!(ids(&filtered(&rows, "VIM")), [1]);
        assert_eq!(ids(&filtered(&rows, "cat")), [2]);
        assert!(filtered(&rows, "emacs").is_empty());
    }

    #[test]
    fn letters_in_order_match_even_apart() {
        let rows = [row(1, "vim notes.md", "~")];
        assert_eq!(ids(&filtered(&rows, "nmd")), [1]);
        assert!(filtered(&rows, "dmn").is_empty());
    }

    #[test]
    fn letters_together_come_before_letters_apart() {
        let rows = [row(1, "cargo test", "~"), row(2, "cat", "~")];
        assert_eq!(ids(&filtered(&rows, "cat")), [2, 1]);
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

    #[test]
    fn states_are_told_in_words() {
        assert_eq!(describe(State::Shown(1)), "Open in a window");
        assert_eq!(describe(State::Shown(3)), "Open in 3 windows");
        assert_eq!(
            describe(State::Background(Duration::from_secs(20))),
            "In the background"
        );
        assert_eq!(
            describe(State::Background(Duration::from_secs(5 * 60))),
            "In the background for 5 min"
        );
        assert_eq!(
            describe(State::Background(Duration::from_secs(3 * 3600 + 60))),
            "In the background for 3 h"
        );
        assert_eq!(describe(State::Ended), "Ended");
    }
}
