//! What a window is called.

use std::path::Path;

/// The title of a window whose shell last reported being in `cwd` (`OSC 7`,
/// as reported), behind the settings' `window.title_prefix`: the
/// directory's name alone, `~` for `home`. Without a directory, what the
/// program last called itself (`OSC 0` or `OSC 2`), `reported`.
pub fn title(
    prefix: Option<&str>,
    cwd: Option<&str>,
    reported: Option<&str>,
    home: Option<&Path>,
) -> String {
    let dir = cwd.and_then(reported_dir).map(|dir| named(&dir, home));
    let reported = reported.filter(|title| !title.is_empty());
    let shown = dir
        .or_else(|| reported.map(str::to_owned))
        .unwrap_or_else(|| "Cattery".to_owned());
    felis_client_core::config::compose_window_title(prefix, &shown)
}

/// The name a directory is called by: its last part, `~` for `home`, and
/// `/` for the root.
fn named(dir: &str, home: Option<&Path>) -> String {
    let path = Path::new(dir);
    if home.is_some_and(|home| path == home) {
        return "~".to_owned();
    }
    path.file_name().map_or_else(
        || dir.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The directory an `OSC 7` report names, whichever machine it is on:
/// the title says where the shell is, even over ssh.
fn reported_dir(reported: &str) -> Option<String> {
    let (_, path) = reported.strip_prefix("file://")?.split_once('/')?;
    let bytes = path.as_bytes();
    let mut decoded = vec![b'/'];
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                at += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

/// `dir` with `home` written as `~`.
pub fn shortened(dir: &str, home: Option<&Path>) -> String {
    match home.and_then(|home| Path::new(dir).strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> Option<&'static Path> {
        Some(Path::new("/home/me"))
    }

    #[test]
    fn a_window_is_called_by_its_shell_s_directory_s_name_alone() {
        assert_eq!(
            title(
                None,
                Some("file://host/home/me/src/cattery"),
                Some("vim notes.md"),
                home()
            ),
            "cattery"
        );
    }

    #[test]
    fn the_home_directory_is_a_tilde_and_the_root_a_slash() {
        assert_eq!(title(None, Some("file://host/home/me"), None, home()), "~");
        assert_eq!(title(None, Some("file://host/"), None, home()), "/");
    }

    #[test]
    fn a_directory_named_as_home_is_but_elsewhere_is_called_by_its_name() {
        assert_eq!(
            title(None, Some("file://host/srv/home/me"), None, home()),
            "me"
        );
    }

    #[test]
    fn without_a_directory_the_program_s_own_title_is_shown() {
        assert_eq!(
            title(None, None, Some("vim notes.md"), home()),
            "vim notes.md"
        );
    }

    #[test]
    fn a_window_whose_program_said_nothing_is_called_cattery() {
        assert_eq!(title(None, None, None, home()), "Cattery");
        assert_eq!(title(None, None, Some(""), home()), "Cattery");
    }

    #[test]
    fn the_prefix_comes_first() {
        assert_eq!(
            title(Some("[work]"), None, Some("zsh"), home()),
            "[work] zsh"
        );
        assert_eq!(title(Some("[work]"), None, None, home()), "[work] Cattery");
    }

    #[test]
    fn a_reported_directory_is_read_however_it_is_escaped() {
        assert_eq!(
            reported_dir("file://host/home/me/%E7%8C%AB%20box").as_deref(),
            Some("/home/me/猫 box")
        );
        assert_eq!(reported_dir("/home/me"), None);
    }

    #[test]
    fn the_home_directory_is_written_as_a_tilde() {
        assert_eq!(shortened("/home/me/src", home()), "~/src");
        assert_eq!(shortened("/home/me", home()), "~");
        assert_eq!(shortened("/home/meg", home()), "/home/meg");
    }
}
