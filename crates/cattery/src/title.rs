//! What a window is called.

/// The title of a window whose program last called itself `reported`
/// (`OSC 0` or `OSC 2`), behind the settings' `window.title_prefix`.
pub fn title(prefix: Option<&str>, reported: Option<&str>) -> String {
    let reported = reported
        .filter(|title| !title.is_empty())
        .unwrap_or("Cattery");
    felis_client_core::config::compose_window_title(prefix, reported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_is_called_what_its_program_calls_it() {
        assert_eq!(title(None, Some("vim notes.md")), "vim notes.md");
    }

    #[test]
    fn a_window_whose_program_said_nothing_is_called_cattery() {
        assert_eq!(title(None, None), "Cattery");
        assert_eq!(title(None, Some("")), "Cattery");
    }

    #[test]
    fn the_prefix_comes_first() {
        assert_eq!(title(Some("[work]"), Some("zsh")), "[work] zsh");
        assert_eq!(title(Some("[work]"), None), "[work] Cattery");
    }
}
