//! Matching what is typed in the palette against what it lists, as fzf
//! reads a query: words in any order, `'exact`, `^start`, `end$` and
//! `!not`, ignoring case until the query has a capital.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// A query, ready to match.
pub struct Query {
    pattern: Pattern,
    matcher: Matcher,
    empty: bool,
}

/// How well a text matched, and which of its characters did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub score: u32,
    /// Character positions in the text, ascending.
    pub marks: Vec<usize>,
}

impl Query {
    pub fn new(query: &str) -> Self {
        Self {
            pattern: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            empty: query.trim().is_empty(),
        }
    }

    /// Whether nothing was typed, which every text matches.
    pub fn is_empty(&self) -> bool {
        self.empty
    }

    /// How `text` matches; `None` where it does not.
    pub fn find(&mut self, text: &str) -> Option<Found> {
        if self.empty {
            return Some(Found {
                score: 0,
                marks: Vec::new(),
            });
        }
        let mut chars = Vec::new();
        let mut marks = Vec::new();
        let score = self.pattern.indices(
            Utf32Str::new(text, &mut chars),
            &mut self.matcher,
            &mut marks,
        )?;
        marks.sort_unstable();
        marks.dedup();
        Some(Found {
            score,
            marks: marks
                .into_iter()
                .filter_map(|mark| usize::try_from(mark).ok())
                .collect(),
        })
    }
}

/// The positions of `items` that match, best first: by score, then the
/// shorter text, then as given.
pub fn ranked<'a>(
    query: &mut Query,
    items: impl IntoIterator<Item = &'a str>,
) -> Vec<(usize, Found)> {
    let mut found: Vec<(usize, usize, Found)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(at, text)| Some((at, text.chars().count(), query.find(text)?)))
        .collect();
    if !query.is_empty() {
        found.sort_by(|a, b| {
            b.2.score
                .cmp(&a.2.score)
                .then(a.1.cmp(&b.1))
                .then(a.0.cmp(&b.0))
        });
    }
    found
        .into_iter()
        .map(|(at, _, found)| (at, found))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(query: &str, items: &[&str]) -> Vec<usize> {
        ranked(&mut Query::new(query), items.iter().copied())
            .into_iter()
            .map(|(at, _)| at)
            .collect()
    }

    #[test]
    fn nothing_typed_matches_everything_in_order() {
        assert_eq!(order("", &["b", "a"]), [0, 1]);
    }

    #[test]
    fn letters_in_order_match_even_apart() {
        assert_eq!(order("nwh", &["New Window Here", "Find…"]), [0]);
        assert!(order("hwn", &["New Window Here"]).is_empty());
    }

    #[test]
    fn a_closer_match_comes_first() {
        assert_eq!(order("cat", &["cargo test", "cat"]), [1, 0]);
    }

    #[test]
    fn words_match_in_any_order_and_fzf_s_marks_apply() {
        assert_eq!(order("window new", &["New Window", "Close Window"]), [0]);
        assert_eq!(order("!close window", &["New Window", "Close Window"]), [0]);
        assert_eq!(order("^close", &["Close Window", "Not close"]), [0]);
    }

    #[test]
    fn case_is_ignored_until_a_capital_is_typed() {
        assert_eq!(order("vim", &["VIM"]), [0]);
        assert!(order("Vim", &["vim"]).is_empty());
    }

    #[test]
    fn the_matched_letters_are_marked() {
        let found = Query::new("nw").find("New Window").unwrap();
        assert_eq!(found.marks, [0, 4]);
    }
}
