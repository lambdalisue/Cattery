//! Cattery's own felis daemon: serving it, and starting it when none
//! answers.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use felis_client_core::{ConnectError, Connection, Offer};
use felis_daemon::SessionPool;
use felis_daemon::serve::DaemonCaps;
use felis_transport::{RetryPolicy, retry_while};
use tokio::sync::Mutex;

use crate::terminfo;

/// The subcommand of the `cattery` executable that serves the daemon.
pub const SUBCOMMAND: &str = "daemon";

/// Serve sessions at `socket` until the daemon is told to stop.
pub async fn serve(socket: &Path) -> anyhow::Result<()> {
    let pool = Arc::new(Mutex::new(SessionPool::new()));
    let caps = DaemonCaps {
        terminfo_dir: terminfo_dir(),
        ..DaemonCaps::default()
    };
    felis_daemon::serve_unix(socket, caps, pool).await?;
    Ok(())
}

/// Where sessions find the terminfo entry their `TERM` names: shipped
/// beside the executable in a package, or compiled into Cattery's cache.
fn terminfo_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().and_then(std::fs::canonicalize).ok();
    if let Some(dir) = exe.as_deref().and_then(terminfo::shipped) {
        return Some(dir);
    }
    let cache = crate::folders::project()?;
    let dir = terminfo::prepared(cache.cache_dir(), felis_daemon::TERMINFO_SOURCE);
    if dir.is_none() && cfg!(unix) {
        tracing::warn!("the terminfo entry was not compiled; programs may not know the terminal");
    }
    dir
}

/// A window's connection to the daemon at `socket`, starting `exe` as the
/// daemon first when nothing listens there.
pub async fn connect(socket: &Path, exe: &Path) -> anyhow::Result<Connection> {
    let window = || felis_client_core::connect(socket, Offer::window(false).with_image_regions());
    match window().await {
        Ok(conn) => return Ok(conn),
        // Anything but an absent listener means a daemon holds the
        // socket; starting another beside it would fight over it.
        Err(err) if !err.may_be_a_cold_socket() => return Err(err.into()),
        Err(err) => tracing::debug!(?err, ?socket, "no daemon answers; starting one"),
    }
    command(exe, socket).spawn()?;
    Ok(retry_while(window, RetryPolicy::DAEMON_BOOT, ConnectError::is_transient).await?)
}

/// How `exe` is started to serve `socket`, apart from whoever started it.
pub fn command(exe: &Path, socket: &Path) -> Command {
    let mut command = Command::new(exe);
    command
        .arg(SUBCOMMAND)
        .arg("--socket")
        .arg(socket)
        // An inherited stderr outlives the window that started the daemon;
        // a write to it after that window closes fails.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // A Cattery started from inside a session carries that session's
        // identity, which the daemon would hand to every session it holds.
        .env_remove("FELIS_SOCKET")
        .env_remove("FELIS_SESSION_ID")
        // A launcher inside a systemd `Type=notify` unit must not let the
        // daemon report readiness in that unit's place.
        .env_remove("NOTIFY_SOCKET");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Out of the launcher's process group, the daemon does not take
        // the Ctrl+C or hangup a terminal sends that group.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // A hidden console of its own, so closing the launcher's console
        // does not end it. CREATE_NEW_PROCESS_GROUP would instead disable
        // Ctrl+C in every shell it starts.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn the_daemon_is_started_as_the_daemon_subcommand_on_the_socket() {
        let command = command(Path::new("/opt/cattery"), Path::new("/tmp/x/daemon.sock"));
        assert_eq!(command.get_program(), "/opt/cattery");
        let args: Vec<&OsStr> = command.get_args().collect();
        assert_eq!(args, ["daemon", "--socket", "/tmp/x/daemon.sock"]);
    }

    #[test]
    fn the_daemon_does_not_inherit_the_identity_of_another_daemon_s_session() {
        let command = command(Path::new("/opt/cattery"), Path::new("/tmp/x/daemon.sock"));
        let removed: Vec<&OsStr> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key)
            .collect();
        for name in ["FELIS_SOCKET", "FELIS_SESSION_ID", "NOTIFY_SOCKET"] {
            assert!(removed.contains(&OsStr::new(name)), "{name} is inherited");
        }
    }
}
