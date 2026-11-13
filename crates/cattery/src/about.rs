//! What Cattery says about itself, and how it shows what it points at.

use std::path::PathBuf;
use std::process::Command;

/// Where Cattery's source is.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The version this build is.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The felis revision this build is pinned to.
pub fn felis() -> Option<&'static str> {
    felis_revision(include_str!("../../../Cargo.toml"))
}

/// The revision the first felis dependency in `manifest` names.
fn felis_revision(manifest: &str) -> Option<&str> {
    let pin = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("felis-"))?;
    let (_, after) = pin.split_once("rev = \"")?;
    after.split_once('"').map(|(rev, _)| rev)
}

/// Something to show the user, in the app the platform shows it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A text file, in the text editor.
    Edit(PathBuf),
    /// A file, selected in the file manager.
    Reveal(PathBuf),
    /// A folder, in the file manager.
    Folder(PathBuf),
    /// A web page, in the browser.
    Url(String),
}

/// The command that shows `target`.
pub fn command(target: &Target) -> Command {
    if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        match target {
            Target::Edit(file) => command.arg("-t").arg(file),
            Target::Reveal(file) => command.arg("-R").arg(file),
            Target::Folder(dir) => command.arg(dir),
            Target::Url(url) => command.arg(url),
        };
        command
    } else if cfg!(target_os = "windows") {
        match target {
            Target::Edit(file) => {
                let mut command = Command::new("notepad");
                command.arg(file);
                command
            }
            Target::Reveal(file) => {
                let mut command = Command::new("explorer");
                command.arg(format!("/select,{}", file.display()));
                command
            }
            Target::Folder(dir) => {
                let mut command = Command::new("explorer");
                command.arg(dir);
                command
            }
            Target::Url(url) => {
                // `start` is the shell's, and its first quoted argument is
                // the window title.
                let mut command = Command::new("cmd");
                command.args(["/C", "start", ""]).arg(url);
                command
            }
        }
    } else {
        // Freedesktop has no way to select a file in the file manager, so a
        // file is shown by the folder it is in.
        let mut command = Command::new("xdg-open");
        match target {
            Target::Edit(file) => command.arg(file),
            Target::Reveal(file) => command.arg(file.parent().unwrap_or(file)),
            Target::Folder(dir) => command.arg(dir),
            Target::Url(url) => command.arg(url),
        };
        command
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    fn argv(command: &Command) -> Vec<&OsStr> {
        std::iter::once(command.get_program())
            .chain(command.get_args())
            .collect()
    }

    #[test]
    fn the_felis_revision_is_read_from_the_pin() {
        let manifest = "[workspace.dependencies]\n\
            anyhow = \"1\"\n\
            felis-daemon = { git = \"https://example.com/felis\", rev = \"abc123\" }\n\
            felis-grid = { git = \"https://example.com/felis\", rev = \"abc123\" }\n";
        assert_eq!(felis_revision(manifest), Some("abc123"));
    }

    #[test]
    fn no_pin_names_no_revision() {
        assert_eq!(felis_revision("[dependencies]\nanyhow = \"1\"\n"), None);
    }

    #[test]
    fn this_build_names_its_felis() {
        assert!(felis().is_some_and(|rev| rev.len() == 40));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_shows_each_target_as_finder_and_its_apps_do() {
        let file = PathBuf::from("/x/config.toml");
        assert_eq!(
            argv(&command(&Target::Edit(file.clone()))),
            ["open", "-t", "/x/config.toml"]
        );
        assert_eq!(
            argv(&command(&Target::Reveal(file))),
            ["open", "-R", "/x/config.toml"]
        );
        assert_eq!(argv(&command(&Target::Folder("/x".into()))), ["open", "/x"]);
        assert_eq!(
            argv(&command(&Target::Url("https://example.com".into()))),
            ["open", "https://example.com"]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_shows_a_file_by_its_folder() {
        let file = PathBuf::from("/x/config.toml");
        assert_eq!(
            argv(&command(&Target::Reveal(file.clone()))),
            ["xdg-open", "/x"]
        );
        assert_eq!(
            argv(&command(&Target::Edit(file))),
            ["xdg-open", "/x/config.toml"]
        );
    }
}
