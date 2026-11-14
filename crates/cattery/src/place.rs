//! Where a new window's session starts.

use std::path::{Path, PathBuf};

/// Where a window opened on `path` starts: the folder itself, or the one
/// holding a file, from the current directory where it is relative; none
/// where nothing is there.
pub fn opened(path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    if path.is_dir() {
        Some(path)
    } else {
        path.parent().map(Path::to_path_buf)
    }
}

/// Where a window opened "here" starts: the directory the shell in the
/// window it was opened from last reported (`OSC 7`), when that names one
/// on this machine.
pub fn here(reported: Option<&str>) -> Option<PathBuf> {
    let dir = felis_client_core::pipe::local_cwd_from_osc7(reported?);
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

/// Where a new window starts: the user's home directory.
pub fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_directory_the_shell_reported_is_here() {
        let dir = tempfile::tempdir().unwrap();
        let reported = format!("file://{}", dir.path().display());
        assert_eq!(here(Some(&reported)), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn nothing_reported_is_nowhere() {
        assert_eq!(here(None), None);
    }

    #[test]
    fn a_directory_on_another_machine_is_not_here() {
        assert_eq!(here(Some("file://elsewhere.invalid/tmp")), None);
    }

    #[test]
    fn a_directory_no_longer_there_is_not_here() {
        assert_eq!(here(Some("file:///no/such/directory")), None);
    }

    #[test]
    fn a_folder_opened_is_where_the_window_starts() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        assert_eq!(opened(dir.path()), Some(canonical));
    }

    #[test]
    fn a_file_opened_starts_the_window_in_the_folder_holding_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.md");
        std::fs::write(&file, "").unwrap();
        assert_eq!(opened(&file), Some(dir.path().canonicalize().unwrap()));
    }

    #[test]
    fn a_relative_path_opened_is_taken_from_the_current_directory() {
        let current = std::env::current_dir().unwrap();
        assert_eq!(
            opened(Path::new(".")),
            Some(current.canonicalize().unwrap())
        );
    }

    #[test]
    fn nothing_there_opens_nowhere() {
        assert_eq!(opened(Path::new("/no/such/directory")), None);
    }

    #[test]
    fn home_is_the_user_s_home() {
        assert_eq!(home(), std::env::home_dir());
    }
}
