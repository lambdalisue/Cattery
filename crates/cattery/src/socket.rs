//! Where Cattery's own felis daemon listens.

use std::ffi::OsString;
use std::path::PathBuf;

/// Names the socket to use instead of the default, for tests and for
/// running two catteries side by side.
pub const OVERRIDE_VAR: &str = "CATTERY_SOCKET";

/// The socket of Cattery's daemon: `overridden` when it is set and not
/// empty, otherwise the user's default.
///
/// `FELIS_SOCKET` is deliberately not read: every shell inside a session
/// has it set to the daemon holding that session, and a cattery started
/// from such a shell would otherwise attach to that daemon.
pub fn resolve(overridden: Option<OsString>) -> anyhow::Result<PathBuf> {
    match overridden {
        Some(path) if !path.is_empty() => Ok(PathBuf::from(path)),
        _ => default(),
    }
}

#[cfg(unix)]
fn default() -> anyhow::Result<PathBuf> {
    Ok(for_user(rustix::process::geteuid().as_raw()))
}

#[cfg(windows)]
fn default() -> anyhow::Result<PathBuf> {
    Ok(from_felis_pipe(
        &felis_transport::local::default_pipe_name()?
    ))
}

#[cfg(unix)]
fn for_user(uid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/cattery.{uid}/daemon.sock"))
}

#[cfg(windows)]
fn from_felis_pipe(felis: &str) -> PathBuf {
    // felis names the pipe after the user's SID and integrity level, which
    // keeps users and elevations apart; only the program's name differs.
    PathBuf::from(felis.replacen(r"\\.\pipe\felis.", r"\\.\pipe\cattery.", 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_names_the_socket() {
        let got = resolve(Some(OsString::from("/elsewhere/cattery.sock"))).unwrap();
        assert_eq!(got, PathBuf::from("/elsewhere/cattery.sock"));
    }

    #[test]
    fn an_empty_override_is_no_override() {
        assert_eq!(
            resolve(Some(OsString::new())).unwrap(),
            resolve(None).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_socket_lives_in_a_directory_of_the_user_s_own() {
        assert_eq!(for_user(501), PathBuf::from("/tmp/cattery.501/daemon.sock"));
    }

    #[cfg(unix)]
    #[test]
    fn the_default_is_the_current_user_s() {
        let uid = rustix::process::geteuid().as_raw();
        assert_eq!(resolve(None).unwrap(), for_user(uid));
    }

    #[cfg(windows)]
    #[test]
    fn the_pipe_is_felis_s_pipe_named_for_cattery() {
        assert_eq!(
            from_felis_pipe(r"\\.\pipe\felis.S-1-5-21-1.daemon"),
            PathBuf::from(r"\\.\pipe\cattery.S-1-5-21-1.daemon"),
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_integrity_level_is_kept() {
        assert_eq!(
            from_felis_pipe(r"\\.\pipe\felis.S-1-5-21-1.high.daemon"),
            PathBuf::from(r"\\.\pipe\cattery.S-1-5-21-1.high.daemon"),
        );
    }
}
