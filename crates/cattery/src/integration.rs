//! Shell integration: the shell marking its prompts and reporting where it
//! is, which zsh is set up to do as Cattery starts it.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};

const ZSHENV: &str = include_str!("../shell/.zshenv");
const ZSH: &str = include_str!("../shell/cattery.zsh");

/// The folder under `cache` zsh is pointed at, written there when it is not
/// there yet; `None` where it cannot be written.
pub fn prepared(cache: &Path) -> Option<PathBuf> {
    // Named after the scripts, so a Cattery with others writes anew beside
    // those a running older one still reads.
    let dir = cache.join("zsh").join(format!("{:016x}", {
        let mut hasher = DefaultHasher::new();
        (ZSHENV, ZSH).hash(&mut hasher);
        hasher.finish()
    }));
    if dir.join("cattery.zsh").is_file() {
        return Some(dir);
    }
    let written = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(dir.join(".zshenv"), ZSHENV))
        .and_then(|()| std::fs::write(dir.join("cattery.zsh"), ZSH));
    match written {
        Ok(()) => Some(dir),
        Err(err) => {
            tracing::debug!(?err, ?dir, "the shell integration was not written");
            _ = std::fs::remove_dir_all(&dir);
            None
        }
    }
}

/// The folder zsh is pointed at, in Cattery's cache; written on first use.
pub fn zsh_dir() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let cache = directories::ProjectDirs::from("", "", "cattery")?;
        prepared(cache.cache_dir())
    })
    .clone()
}

/// Whether `program` is zsh, by the name it is run by.
pub fn is_zsh(program: &str) -> bool {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program);
    let name = name.trim_start_matches('-');
    name == "zsh" || name.starts_with("zsh-")
}

/// What to add to zsh's environment to load the integration in `dir`, the
/// user's own `ZDOTDIR` being `zdotdir`.
pub fn zsh_env(dir: &Path, zdotdir: Option<&str>) -> Vec<(String, String)> {
    let mut env = vec![
        ("ZDOTDIR".to_owned(), dir.display().to_string()),
        (
            "CATTERY_SHELL_INTEGRATION".to_owned(),
            dir.join("cattery.zsh").display().to_string(),
        ),
    ];
    if let Some(zdotdir) = zdotdir {
        env.push(("CATTERY_ZDOTDIR".to_owned(), zdotdir.to_owned()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_is_known_however_it_is_named() {
        assert!(is_zsh("zsh"));
        assert!(is_zsh("/bin/zsh"));
        assert!(is_zsh("-zsh"));
        assert!(is_zsh("/opt/homebrew/bin/zsh-5.9"));
        assert!(!is_zsh("/usr/bin/fish"));
        assert!(!is_zsh("zshdb"));
    }

    #[test]
    fn the_user_s_zdotdir_is_handed_on_only_where_they_have_one() {
        let dir = Path::new("/cache/zsh/1");
        let keys = |env: Vec<(String, String)>| -> Vec<String> {
            env.into_iter().map(|(key, _)| key).collect()
        };
        assert_eq!(
            keys(zsh_env(dir, None)),
            ["ZDOTDIR", "CATTERY_SHELL_INTEGRATION"]
        );
        assert!(zsh_env(dir, Some("/home/me/.config/zsh")).contains(&(
            "CATTERY_ZDOTDIR".to_owned(),
            "/home/me/.config/zsh".to_owned()
        )));
    }

    #[test]
    fn the_scripts_are_written_once_into_the_cache() {
        let cache = tempfile::tempdir().unwrap();
        let dir = prepared(cache.path()).unwrap();
        assert!(dir.join(".zshenv").is_file());
        assert_eq!(
            std::fs::read_to_string(dir.join("cattery.zsh")).unwrap(),
            ZSH
        );
        assert_eq!(prepared(cache.path()), Some(dir));
    }

    /// zsh started as Cattery starts it, with `home` as HOME; what it prints.
    #[cfg(unix)]
    fn zsh(home: &Path, zdotdir: Option<&Path>, script: &str) -> Option<String> {
        let cache = tempfile::tempdir().unwrap();
        let dir = prepared(cache.path()).unwrap();
        let mut command = std::process::Command::new("zsh");
        command
            .args(["-c", script])
            .env("HOME", home)
            .env_remove("ZDOTDIR")
            .envs(zsh_env(&dir, zdotdir.and_then(Path::to_str)));
        let out = command.output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    #[cfg(unix)]
    #[test]
    fn zsh_reads_the_user_s_own_startup_files_as_ever() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join(".zshenv"), "mine=home\n").unwrap();
        let Some(out) = zsh(home.path(), None, "print -r -- $mine ${+ZDOTDIR}") else {
            return;
        };
        assert_eq!(out, "home 0\n");
    }

    #[cfg(unix)]
    #[test]
    fn an_interactive_zsh_marks_its_prompts_and_how_each_command_ended() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let home = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let dir = prepared(cache.path()).unwrap();
        let Ok(mut shell) = Command::new("zsh")
            .arg("-i")
            .env("HOME", home.path())
            .env_remove("ZDOTDIR")
            .envs(zsh_env(&dir, None))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return;
        };
        shell
            .stdin
            .take()
            .unwrap()
            .write_all(b"false\nexit\n")
            .unwrap();
        let out = shell.wait_with_output().unwrap();
        let printed = String::from_utf8_lossy(&out.stderr).into_owned()
            + &String::from_utf8_lossy(&out.stdout);
        for mark in [
            "\x1b]133;A",
            "\x1b]133;B",
            "\x1b]133;C",
            "\x1b]133;D;1",
            "\x1b]7;file://",
        ] {
            assert!(printed.contains(mark), "{mark:?} in {printed:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn zsh_reads_them_from_the_user_s_zdotdir_where_they_set_one() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("zsh");
        std::fs::create_dir(&config).unwrap();
        std::fs::write(config.join(".zshenv"), "mine=zdotdir\n").unwrap();
        let script = "print -r -- $mine $ZDOTDIR ${+CATTERY_ZDOTDIR}";
        let Some(out) = zsh(home.path(), Some(&config), script) else {
            return;
        };
        assert_eq!(out, format!("zdotdir {} 0\n", config.display()));
    }
}
