//! Where a new window's session starts.

use std::path::PathBuf;

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
    fn home_is_the_user_s_home() {
        assert_eq!(home(), std::env::home_dir());
    }
}
