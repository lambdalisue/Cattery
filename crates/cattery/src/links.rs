//! Links in a session's screen: those a program marked with OSC 8, and
//! URLs written out as plain text.

use felis_client_core::{ActivationTarget, GridPos, ShadowScreen};
use felis_grid::Grapheme;

/// The link at `cell`, checked as safe to hand to the platform: the one a
/// program marked there, or else a URL written across it.
pub fn at(shadow: &ShadowScreen, cell: GridPos) -> Option<ActivationTarget> {
    let screen = shadow.screen();
    let marked = screen
        .cell(cell.row, cell.col)
        .and_then(|cell| cell.link)
        .and_then(|id| shadow.hyperlink(id))
        .map(|entry| entry.uri.as_str().to_owned());
    let uri = marked.or_else(|| {
        let row: Vec<(u16, String)> = (0..screen.cols())
            .filter_map(|col| {
                let text = match screen.cell(cell.row, col)?.grapheme {
                    Grapheme::Empty => " ".to_owned(),
                    Grapheme::Ascii(byte) => char::from(byte).to_string(),
                    Grapheme::Char(letter) => letter.to_string(),
                    Grapheme::Cluster(id) => screen.cluster_str(id)?.to_owned(),
                    Grapheme::Spacer | Grapheme::SizedSpacer => return None,
                };
                Some((col, text))
            })
            .collect();
        written(&row, cell.col)
    })?;
    ActivationTarget::parse(&uri).ok()
}

/// The schemes a URL written as plain text is recognised by.
const SCHEMES: [&str; 3] = ["https://", "http://", "file://"];

/// The URL written across column `col` of a row, given as each cell's
/// column and text.
fn written(row: &[(u16, String)], col: u16) -> Option<String> {
    let at = row.iter().position(|(each, _)| *each >= col)?;
    if row[at].0 != col {
        return None;
    }
    let inside = |text: &str| {
        text.chars()
            .all(|letter| !letter.is_whitespace() && !"<>\"'`".contains(letter))
    };
    if !inside(&row[at].1) {
        return None;
    }
    let mut start = at;
    while start > 0 && inside(&row[start - 1].1) {
        start -= 1;
    }
    let mut end = at + 1;
    while end < row.len() && inside(&row[end].1) {
        end += 1;
    }
    let word: String = row[start..end]
        .iter()
        .map(|(_, text)| text.as_str())
        .collect();
    // How far into the word the URL starts, and how far into it `col` is.
    let offset = SCHEMES
        .iter()
        .filter_map(|scheme| word.find(scheme))
        .min()?;
    let before: usize = row[start..at].iter().map(|(_, text)| text.len()).sum();
    if before < offset {
        return None;
    }
    let url = trimmed(&word[offset..]);
    if url.len()
        <= SCHEMES
            .iter()
            .find(|scheme| url.starts_with(**scheme))?
            .len()
        || before >= offset + url.len()
    {
        return None;
    }
    Some(url.to_owned())
}

/// `url` without the punctuation that ends the sentence it is written in,
/// and without a closing bracket it does not open.
fn trimmed(url: &str) -> &str {
    let mut url = url;
    loop {
        let Some(last) = url.chars().last() else {
            return url;
        };
        let unopened = match last {
            ')' => url.matches('(').count() < url.matches(')').count(),
            ']' => url.matches('[').count() < url.matches(']').count(),
            '}' => url.matches('{').count() < url.matches('}').count(),
            '.' | ',' | ';' | ':' | '!' | '?' => true,
            _ => false,
        };
        if !unopened {
            return url;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str) -> Vec<(u16, String)> {
        text.chars()
            .enumerate()
            .map(|(col, letter)| (u16::try_from(col).unwrap(), letter.to_string()))
            .collect()
    }

    #[test]
    fn a_url_is_found_from_any_of_its_cells() {
        let row = row("see https://example.com/a?b=c now");
        for col in 4..29 {
            assert_eq!(
                written(&row, col).as_deref(),
                Some("https://example.com/a?b=c"),
                "{col}"
            );
        }
        assert_eq!(written(&row, 3), None);
        assert_eq!(written(&row, 29), None);
    }

    #[test]
    fn text_glued_before_the_scheme_is_not_the_url() {
        let row = row("x=https://example.com");
        assert_eq!(written(&row, 0), None);
        assert_eq!(written(&row, 2).as_deref(), Some("https://example.com"));
    }

    #[test]
    fn the_sentence_s_punctuation_is_left_out() {
        assert_eq!(
            written(&row("(see https://example.com/a)."), 6).as_deref(),
            Some("https://example.com/a")
        );
        assert_eq!(
            written(
                &row("https://en.wikipedia.org/wiki/Cat_(disambiguation)"),
                0
            )
            .as_deref(),
            Some("https://en.wikipedia.org/wiki/Cat_(disambiguation)")
        );
    }

    #[test]
    fn a_scheme_alone_is_no_url() {
        assert_eq!(written(&row("https:// x"), 0), None);
    }

    #[test]
    fn words_that_are_no_url_hold_none() {
        assert_eq!(written(&row("cargo test --all"), 2), None);
    }

    #[test]
    fn a_wide_letter_counts_once_however_many_columns_it_takes() {
        let row = vec![
            (0, "猫".to_owned()),
            (2, " ".to_owned()),
            (3, "h".to_owned()),
        ]
        .into_iter()
        .chain(
            row("ttps://例.jp")
                .into_iter()
                .map(|(col, text)| (col + 4, text)),
        )
        .collect::<Vec<_>>();
        assert_eq!(written(&row, 5).as_deref(), Some("https://例.jp"));
        assert_eq!(written(&row, 1), None);
    }
}
