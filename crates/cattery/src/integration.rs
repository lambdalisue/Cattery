//! Shell integration: the shell marking its prompts, reporting where it
//! is and what it ran, which zsh and fish are set up to do as Cattery
//! starts them.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};

const ZSHENV: &str = include_str!("../shell/.zshenv");
const ZSH: &str = include_str!("../shell/cattery.zsh");
const FISH: &str = include_str!("../shell/fish/vendor_conf.d/cattery.fish");

/// The scripts, where they go in the folder.
const SCRIPTS: [(&str, &str); 3] = [
    (".zshenv", ZSHENV),
    ("cattery.zsh", ZSH),
    ("fish/vendor_conf.d/cattery.fish", FISH),
];

/// The folder under `cache` the shells are pointed at, written there when it is not
/// there yet; `None` where it cannot be written.
pub fn prepared(cache: &Path) -> Option<PathBuf> {
    // Named after the scripts, so a Cattery with others writes anew beside
    // those a running older one still reads.
    let dir = cache.join("shell").join(format!("{:016x}", {
        let mut hasher = DefaultHasher::new();
        SCRIPTS.hash(&mut hasher);
        hasher.finish()
    }));
    if SCRIPTS.iter().all(|(path, _)| dir.join(path).is_file()) {
        return Some(dir);
    }
    let written = SCRIPTS.iter().try_for_each(|(path, script)| {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap_or(&dir))?;
        std::fs::write(path, script)
    });
    match written {
        Ok(()) => Some(dir),
        Err(err) => {
            tracing::debug!(?err, ?dir, "the shell integration was not written");
            _ = std::fs::remove_dir_all(&dir);
            None
        }
    }
}

/// The folder the shells are pointed at, in Cattery's cache; written on
/// first use.
pub fn dir() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let cache = crate::folders::project()?;
        prepared(cache.cache_dir())
    })
    .clone()
}

/// Whether `program` is the shell `shell`, by the name it is run by.
pub fn is(program: &str, shell: &str) -> bool {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program);
    let name = name.trim_start_matches('-');
    name == shell
        || name
            .strip_prefix(shell)
            .is_some_and(|rest| rest.starts_with('-'))
}

/// What to add to the environment of `program` to load the integration in
/// `dir`, `env` reading the environment Cattery has; nothing for a shell
/// with none.
pub fn env(
    program: &str,
    dir: &Path,
    env: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    if is(program, "zsh") {
        zsh_env(dir, env("ZDOTDIR").as_deref())
    } else if is(program, "fish") {
        fish_env(dir, env("XDG_DATA_DIRS").as_deref())
    } else {
        Vec::new()
    }
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

/// What to add to fish's environment to load the integration in `dir`, the
/// user's own `XDG_DATA_DIRS` being `data_dirs`.
pub fn fish_env(dir: &Path, data_dirs: Option<&str>) -> Vec<(String, String)> {
    // Unset, the directories fall back to these, which fish must still
    // search.
    let rest = data_dirs.unwrap_or("/usr/local/share:/usr/share");
    let mut env = vec![(
        "XDG_DATA_DIRS".to_owned(),
        format!("{}:{rest}", dir.display()),
    )];
    if let Some(data_dirs) = data_dirs {
        env.push(("CATTERY_XDG_DATA_DIRS".to_owned(), data_dirs.to_owned()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_is_known_however_it_is_named() {
        assert!(is("zsh", "zsh"));
        assert!(is("/bin/zsh", "zsh"));
        assert!(is("-zsh", "zsh"));
        assert!(is("/opt/homebrew/bin/zsh-5.9", "zsh"));
        assert!(!is("/usr/bin/fish", "zsh"));
        assert!(!is("zshdb", "zsh"));
        assert!(is("/run/current-system/sw/bin/fish", "fish"));
    }

    #[test]
    fn only_zsh_and_fish_are_set_up() {
        let dir = Path::new("/cache/shell/1");
        let none = |_: &str| None;
        assert!(env("/bin/bash", dir, none).is_empty());
        assert_eq!(env("/bin/zsh", dir, none)[0].0, "ZDOTDIR");
        assert_eq!(env("fish", dir, none)[0].0, "XDG_DATA_DIRS");
    }

    #[test]
    fn fish_searches_cattery_s_folder_first_and_the_user_s_after() {
        let dir = Path::new("/cache/shell/1");
        assert_eq!(
            fish_env(dir, None),
            [(
                "XDG_DATA_DIRS".to_owned(),
                "/cache/shell/1:/usr/local/share:/usr/share".to_owned()
            )]
        );
        assert_eq!(
            fish_env(dir, Some("/nix/share")),
            [
                (
                    "XDG_DATA_DIRS".to_owned(),
                    "/cache/shell/1:/nix/share".to_owned()
                ),
                ("CATTERY_XDG_DATA_DIRS".to_owned(), "/nix/share".to_owned()),
            ]
        );
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
            // The command as it runs and as it ended, as felis's program
            // status keeps it: "false" in base64.
            "\x1b]7501;state=working:id=shell:app=cattery:title=ZmFsc2U=",
            "\x1b]7501;state=error:id=shell:app=cattery:title=ZmFsc2U=",
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
