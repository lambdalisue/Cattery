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
