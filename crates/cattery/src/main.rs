use cattery::{app, daemon, settings, socket};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// A GUI terminal app built on the felis terminal daemon.
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the sessions Cattery's windows show. Cattery starts it itself.
    #[command(hide = true)]
    Daemon {
        #[arg(long)]
        socket: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Some(Command::Daemon { socket }) => {
            tokio::runtime::Runtime::new()?.block_on(daemon::serve(&socket))
        }
        None => {
            #[cfg(windows)]
            leave_own_console();
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .with_writer(std::io::stderr)
                .init();
            app::run(
                socket::resolve(std::env::var_os(socket::OVERRIDE_VAR))?,
                settings::path(std::env::var_os(settings::OVERRIDE_VAR)),
            )
        }
    }
}

/// Close the console Windows opened for Cattery alone, as it does for a
/// console program started from Explorer or the Start menu; a console
/// shared with the terminal Cattery was started from is kept.
///
/// Cattery stays a console program rather than a GUI one so that
/// `cattery --version` prints to the terminal, and so that the daemon it
/// starts has a hidden console of its own.
#[cfg(windows)]
fn leave_own_console() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn FreeConsole() -> i32;
    }
    let mut attached = [0_u32; 2];
    // SAFETY: the buffer holds as many ids as the count passed says.
    let count = unsafe { GetConsoleProcessList(attached.as_mut_ptr(), 2) };
    if count == 1 {
        // SAFETY: takes nothing, and Cattery writes nothing to the console
        // once its window is up.
        unsafe { FreeConsole() };
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_daemon_is_started_with_arguments_cattery_takes() {
        let started = daemon::command(Path::new("cattery"), Path::new("/tmp/x/daemon.sock"));
        let argv = std::iter::once(started.get_program()).chain(started.get_args());
        match Cli::try_parse_from(argv).unwrap().command {
            Some(Command::Daemon { socket }) => {
                assert_eq!(socket, PathBuf::from("/tmp/x/daemon.sock"));
            }
            other => panic!("not the daemon: {other:?}"),
        }
    }
}
