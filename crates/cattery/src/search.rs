//! Searching a session's history: what is typed, the matches the daemon
//! finds, and where they are on screen.

use felis_client_core::viewport::{composed_row_for_line, viewport_for_hit};
use felis_protocol::messages::{ColSpan, SearchToClientMsg, ViewportAnchor};
use felis_render_core::overlay::SearchHitSpan;

/// One match: the line it starts on, its spans of columns, and the frame of
/// history its lines are counted in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub line_index: i64,
    pub spans: Vec<ColSpan>,
    pub anchor: Option<ViewportAnchor>,
}

impl From<SearchToClientMsg> for Hit {
    fn from(msg: SearchToClientMsg) -> Self {
        let SearchToClientMsg::Match {
            line_index,
            col_spans,
            anchor,
            ..
        } = msg;
        Self {
            line_index,
            spans: col_spans,
            anchor,
        }
    }
}

/// Which way to step through the matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Back into history.
    Older,
    /// Towards the live screen.
    Newer,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    /// The query is being typed.
    Typing,
    /// The query was sent; its matches arrive and are stepped through.
    Shown,
}

/// A search, from its query to its matches.
#[derive(Debug, Default)]
pub struct Search {
    pub mode: Mode,
    pub query: String,
    hits: Vec<Hit>,
    current: Option<usize>,
    /// Whether matches may still arrive.
    streaming: bool,
    failed: Option<String>,
}

impl Search {
    /// Start typing a query, the last one kept to change.
    pub fn begin(&mut self) {
        self.mode = Mode::Typing;
    }

    /// The query was sent: matches will arrive, newest first.
    pub fn sent(&mut self) {
        self.mode = Mode::Shown;
        self.hits.clear();
        self.current = None;
        self.streaming = true;
        self.failed = None;
    }

    /// A match arrived; the first is the one shown.
    pub fn found(&mut self, hit: Hit) {
        self.hits.push(hit);
        if self.current.is_none() {
            self.current = Some(0);
        }
    }

    /// No more matches arrive: all were sent, or the search failed so.
    pub fn ended(&mut self, failure: Option<String>) {
        self.streaming = false;
        self.failed = failure;
    }

    /// Step to the next match `step` way, wrapping round; the match.
    pub fn step(&mut self, step: Step) -> Option<&Hit> {
        let count = self.hits.len();
        if count == 0 {
            return None;
        }
        self.current = Some(match (self.current, step) {
            (None, Step::Older) => 0,
            (None, Step::Newer) => count - 1,
            (Some(at), Step::Older) => (at + 1) % count,
            (Some(at), Step::Newer) => (at + count - 1) % count,
        });
        self.current()
    }

    pub fn current(&self) -> Option<&Hit> {
        self.current.and_then(|at| self.hits.get(at))
    }

    /// What the search bar says.
    pub fn label(&self) -> String {
        let query = format!("Find: {}", self.query);
        if self.mode != Mode::Shown {
            return query;
        }
        if let Some(failure) = &self.failed {
            return format!("{query}  {failure}");
        }
        let more = if self.streaming { "…" } else { "" };
        match self.current {
            Some(at) => format!("{query}  {} of {}{more}", at + 1, self.hits.len()),
            None if self.streaming => format!("{query}  Searching…"),
            None => format!("{query}  No matches"),
        }
    }

    /// The matches on screen, the view showing `viewport` rows of history
    /// counted in the frame `frame`, `rows` rows tall.
    pub fn spans(
        &self,
        frame: Option<ViewportAnchor>,
        viewport: u32,
        rows: u16,
    ) -> Vec<SearchHitSpan> {
        let mut spans = Vec::new();
        for (at, hit) in self.hits.iter().enumerate() {
            let Some(moved) = shift(hit, frame) else {
                continue;
            };
            for span in hit
                .spans
                .iter()
                .filter(|span| span.col_end > span.col_start)
            {
                if let Some(row) = composed_row_for_line(span.line_index - moved, viewport, rows) {
                    spans.push(SearchHitSpan {
                        row,
                        col_start: span.col_start,
                        col_end: span.col_end,
                        is_current: self.current == Some(at),
                    });
                }
            }
        }
        spans
    }
}

/// How many rows a match's lines have moved since it was found, counted in
/// the view's frame: none where either has no frame, and `None` where the
/// frames are not the same history.
fn shift(hit: &Hit, frame: Option<ViewportAnchor>) -> Option<i64> {
    match hit.anchor.zip(frame) {
        Some((found, shown)) => found.rows_until(shown),
        None => Some(0),
    }
}

/// The view that shows `hit`: how far back into history, in which frame.
pub fn view_of(
    hit: &Hit,
    frame: Option<ViewportAnchor>,
    rows: u16,
    max_scroll: u32,
) -> (u32, Option<ViewportAnchor>) {
    let line = hit
        .spans
        .first()
        .map_or(hit.line_index, |span| span.line_index);
    match shift(hit, frame) {
        // In the frame shown: the line, moved as output moved it.
        Some(moved) if hit.anchor.is_none() || (moved >= 0 && frame.is_some()) => {
            (viewport_for_hit(line - moved, rows, max_scroll), frame)
        }
        // In another frame: as it was found, which the daemon places.
        _ => (viewport_for_hit(line, rows, u32::MAX), hit.anchor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(line_index: i64) -> Hit {
        Hit {
            line_index,
            spans: vec![ColSpan {
                line_index,
                col_start: 2,
                col_end: 5,
            }],
            anchor: None,
        }
    }

    fn shown(hits: usize) -> Search {
        let mut search = Search {
            query: "cargo".to_owned(),
            ..Search::default()
        };
        search.sent();
        for line in 0..hits {
            search.found(hit(-(i64::try_from(line).unwrap()) - 1));
        }
        search
    }

    #[test]
    fn the_newest_match_is_shown_first_and_older_ones_follow() {
        let mut search = shown(3);
        assert_eq!(search.current().map(|hit| hit.line_index), Some(-1));
        assert_eq!(search.step(Step::Older).map(|hit| hit.line_index), Some(-2));
        assert_eq!(search.step(Step::Older).map(|hit| hit.line_index), Some(-3));
    }

    #[test]
    fn stepping_past_either_end_wraps_round() {
        let mut search = shown(3);
        assert_eq!(search.step(Step::Newer).map(|hit| hit.line_index), Some(-3));
        assert_eq!(search.step(Step::Older).map(|hit| hit.line_index), Some(-1));
    }

    #[test]
    fn no_match_steps_nowhere() {
        assert_eq!(shown(0).step(Step::Older), None);
    }

    #[test]
    fn the_bar_says_what_is_being_found_and_how_far_along() {
        let mut typing = Search {
            query: "car".to_owned(),
            ..Search::default()
        };
        typing.begin();
        assert_eq!(typing.label(), "Find: car");
        let mut search = shown(3);
        assert_eq!(search.label(), "Find: cargo  1 of 3…");
        search.ended(None);
        search.step(Step::Older);
        assert_eq!(search.label(), "Find: cargo  2 of 3");
        let mut none = shown(0);
        none.ended(None);
        assert_eq!(none.label(), "Find: cargo  No matches");
        let mut failed = shown(0);
        failed.ended(Some("regex parse error".to_owned()));
        assert_eq!(failed.label(), "Find: cargo  regex parse error");
    }

    #[test]
    fn a_match_on_screen_is_marked_where_it_is() {
        let search = shown(1);
        assert_eq!(
            search.spans(None, 1, 24),
            [SearchHitSpan {
                row: 0,
                col_start: 2,
                col_end: 5,
                is_current: true,
            }]
        );
    }

    #[test]
    fn a_match_out_of_view_is_not_marked() {
        assert!(shown(1).spans(None, 0, 24).is_empty());
    }

    #[test]
    fn a_match_back_in_history_is_brought_into_view() {
        let (offset, frame) = view_of(&hit(-30), None, 24, 1000);
        assert!(offset >= 30, "{offset}");
        assert_eq!(frame, None);
        assert_eq!(view_of(&hit(3), None, 24, 1000).0, 0);
    }
}
