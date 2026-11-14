//! Selecting text with the mouse, and pasting it back.

use std::time::{Duration, Instant};

use felis_client_core::GridPos;
use felis_client_core::confirm::paste_needs_confirmation;
use felis_protocol::limits::MAX_PASTE_BYTES;
use felis_protocol::messages::InputMsg;

/// How soon a click follows the last for the two to make a double click.
/// How soon a click follows the last to count as one of the same row.
pub const CLICK_INTERVAL: Duration = Duration::from_millis(500);

/// The clicks in a row on one cell: one selects from there, two a word,
/// three a line, and a fourth starts over.
#[derive(Debug, Default)]
pub struct Clicks {
    last: Option<(Instant, GridPos)>,
    count: u8,
}

impl Clicks {
    /// Count a click on `cell` at `now`; how many it makes in a row.
    pub fn record(&mut self, now: Instant, cell: GridPos) -> u8 {
        let follows = self.last.is_some_and(|(at, last)| {
            last == cell && now.saturating_duration_since(at) <= CLICK_INTERVAL
        });
        self.count = if follows { self.count % 3 + 1 } else { 1 };
        self.last = Some((now, cell));
        self.count
    }
}

/// Whether a change of screen leaves a selection pointing at other text:
/// the alternate screen came or went, or the view moved through history.
pub fn invalidated(alt_screens: (bool, bool), viewports: (u32, u32)) -> bool {
    alt_screens.0 != alt_screens.1 || viewports.0 != viewports.1
}

/// What pasting text does.
#[derive(Debug, PartialEq, Eq)]
pub enum Paste {
    /// Send it.
    Send(InputMsg),
    /// Ask first: it holds control characters a shell would act on, as
    /// a newline runs a command, and the program does not take pastes as
    /// such.
    Confirm(InputMsg),
    /// It is more than one paste may be.
    TooLarge,
}

/// What pasting `text` does, into a program that takes pastes as such
/// (bracketed paste) or not.
pub fn paste(text: &str, bracketed: bool) -> Paste {
    let bytes = text.as_bytes();
    if bytes.len() > MAX_PASTE_BYTES {
        return Paste::TooLarge;
    }
    // The daemon brackets the paste itself where the program asked.
    let input = InputMsg::Paste(bytes.to_vec());
    if paste_needs_confirmation(bracketed, bytes) {
        Paste::Confirm(input)
    } else {
        Paste::Send(input)
    }
}

/// What dropping the file at `path` on a window pastes: its path as a shell
/// reads one word, and a space to type the next after it.
pub fn dropped(path: &std::path::Path) -> String {
    format!("{} ", shell_words::quote(&path.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: u16, col: u16) -> GridPos {
        GridPos { row, col }
    }

    #[test]
    fn clicks_on_one_cell_in_quick_succession_count_up_and_start_over() {
        let mut clicks = Clicks::default();
        let now = Instant::now();
        let at = |ms| now + Duration::from_millis(ms);
        assert_eq!(clicks.record(at(0), cell(1, 1)), 1);
        assert_eq!(clicks.record(at(200), cell(1, 1)), 2);
        assert_eq!(clicks.record(at(400), cell(1, 1)), 3);
        assert_eq!(clicks.record(at(600), cell(1, 1)), 1);
    }

    #[test]
    fn a_slow_click_or_one_elsewhere_starts_over() {
        let mut clicks = Clicks::default();
        let now = Instant::now();
        assert_eq!(clicks.record(now, cell(1, 1)), 1);
        assert_eq!(
            clicks.record(now + Duration::from_millis(900), cell(1, 1)),
            1
        );
        assert_eq!(
            clicks.record(now + Duration::from_millis(1000), cell(1, 2)),
            1
        );
    }

    #[test]
    fn the_alternate_screen_coming_or_going_drops_the_selection() {
        assert!(invalidated((false, true), (0, 0)));
        assert!(invalidated((true, false), (0, 0)));
        assert!(!invalidated((false, false), (0, 0)));
    }

    #[test]
    fn moving_through_history_drops_the_selection() {
        assert!(invalidated((false, false), (0, 10)));
        assert!(!invalidated((false, false), (10, 10)));
    }

    #[test]
    fn a_dropped_file_is_pasted_as_its_path_and_a_space() {
        assert_eq!(
            dropped(std::path::Path::new("/tmp/notes.md")),
            "/tmp/notes.md "
        );
    }

    #[test]
    fn a_dropped_path_with_spaces_stays_one_word() {
        assert_eq!(
            dropped(std::path::Path::new("/tmp/my notes.md")),
            "'/tmp/my notes.md' "
        );
    }

    #[test]
    fn plain_text_is_pasted_at_once() {
        assert_eq!(
            paste("hello world", false),
            Paste::Send(InputMsg::Paste(b"hello world".to_vec()))
        );
    }

    #[test]
    fn a_line_break_asks_first_unless_the_program_takes_pastes() {
        assert_eq!(
            paste("rm -rf build\n", false),
            Paste::Confirm(InputMsg::Paste(b"rm -rf build\n".to_vec()))
        );
        assert_eq!(
            paste("rm -rf build\n", true),
            Paste::Send(InputMsg::Paste(b"rm -rf build\n".to_vec()))
        );
    }

    #[test]
    fn tabs_are_pasted_at_once() {
        assert!(matches!(paste("a\tb", false), Paste::Send(_)));
    }

    #[test]
    fn too_much_is_not_pasted() {
        assert_eq!(
            paste(&"x".repeat(MAX_PASTE_BYTES + 1), true),
            Paste::TooLarge
        );
    }
}
