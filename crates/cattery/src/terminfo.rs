//! The `xterm-felis` terminfo entry every session's `TERM` names, compiled
//! where the daemon hands it to sessions.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The folder under `cache` holding the entry compiled from `source`,
/// compiling it there with `tic` when it is not there yet. `None` where it
/// cannot be compiled, as on Windows, which has no terminfo.
pub fn prepared(cache: &Path, source: &str) -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    // Named after the source, so a felis with another entry compiles anew
    // beside the entry an older Cattery still uses.
    let dir = cache.join("terminfo").join(format!("{:016x}", {
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    }));
    if holds_entry(&dir) {
        return Some(dir);
    }
    // Compiled in a folder of its own and moved into place whole: several
    // daemons may start at once, and one must never see, or remove, the
    // folder another is still writing.
    let parent = dir.parent()?;
    std::fs::create_dir_all(parent).ok()?;
    let scratch = tempfile::Builder::new()
        .prefix(".compiling-")
        .tempdir_in(parent)
        .ok()?;
    let file = scratch.path().join("xterm-felis.terminfo");
    std::fs::write(&file, source).ok()?;
    let compiled = Command::new(tic())
        .arg("-x")
        .arg("-o")
        .arg(scratch.path())
        .arg(&file)
        .output();
    match compiled {
        Ok(out) if out.status.success() && holds_entry(scratch.path()) => {
            let scratch = scratch.keep();
            // Another daemon may have moved its own into place first; it
            // holds the same entry, compiled from the same source.
            if std::fs::rename(&scratch, &dir).is_err() {
                _ = std::fs::remove_dir_all(&scratch);
            }
            holds_entry(&dir).then_some(dir)
        }
        other => {
            tracing::debug!(?other, ?dir, "the terminfo entry was not compiled");
            None
        }
    }
}

/// The `tic` to compile with. macOS's terminfo readers take only the
/// format its own `tic` writes, which a newer ncurses's does not by default.
fn tic() -> &'static str {
    if cfg!(target_os = "macos") && Path::new("/usr/bin/tic").is_file() {
        "/usr/bin/tic"
    } else {
        "tic"
    }
}

/// The folder a package ships the entry in beside the executable at `exe`,
/// where there is one: `Contents/Resources/terminfo` in a macOS app,
/// `share/terminfo` otherwise.
pub fn shipped(exe: &Path) -> Option<PathBuf> {
    let prefix = exe.parent()?.parent()?;
    [
        prefix.join("Resources/terminfo"),
        prefix.join("share/terminfo"),
    ]
    .into_iter()
    .find(|dir| holds_entry(dir))
}

/// Whether `dir` holds the entry where terminfo readers look: under its
/// first letter, or under that letter's hex code as macOS files it.
fn holds_entry(dir: &Path) -> bool {
    ["x", "78"]
        .into_iter()
        .any(|sub| dir.join(sub).join("xterm-felis").is_file())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn tic() -> bool {
        std::process::Command::new("tic")
            .arg("-V")
            .output()
            .is_ok_and(|out| out.status.success())
    }

    #[test]
    fn an_entry_shipped_beside_the_executable_is_found() {
        let prefix = tempfile::tempdir().unwrap();
        let exe = prefix.path().join("bin/cattery");
        assert_eq!(shipped(&exe), None);
        let dir = prefix.path().join("share/terminfo");
        std::fs::create_dir_all(dir.join("x")).unwrap();
        std::fs::write(dir.join("x/xterm-felis"), "").unwrap();
        assert_eq!(shipped(&exe), Some(dir));
    }

    #[test]
    fn the_entry_is_compiled_where_sessions_look_for_it() {
        if !tic() {
            eprintln!("tic is not installed; skipped");
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let dir = prepared(cache.path(), felis_daemon::TERMINFO_SOURCE).unwrap();
        assert!(dir.starts_with(cache.path()));
        assert!(holds_entry(&dir));
    }

    #[test]
    fn daemons_compiling_at_once_all_end_with_the_entry() {
        if !tic() {
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let compiled: Vec<Option<PathBuf>> = std::thread::scope(|scope| {
            let compiling: Vec<_> = (0..12)
                .map(|_| scope.spawn(|| prepared(cache.path(), felis_daemon::TERMINFO_SOURCE)))
                .collect();
            compiling
                .into_iter()
                .map(|each| each.join().unwrap())
                .collect()
        });
        for dir in &compiled {
            let dir = dir.as_ref().expect("every daemon has the entry");
            assert!(holds_entry(dir), "{dir:?}");
        }
    }

    #[test]
    fn another_entry_is_compiled_apart() {
        if !tic() {
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let first = prepared(cache.path(), felis_daemon::TERMINFO_SOURCE).unwrap();
        let changed = format!("{}\n", felis_daemon::TERMINFO_SOURCE);
        let second = prepared(cache.path(), &changed).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn an_entry_already_compiled_is_used_as_it_is() {
        if !tic() {
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let dir = prepared(cache.path(), felis_daemon::TERMINFO_SOURCE).unwrap();
        let marker = dir.join("kept");
        std::fs::write(&marker, "").unwrap();
        assert_eq!(
            prepared(cache.path(), felis_daemon::TERMINFO_SOURCE).unwrap(),
            dir
        );
        assert!(marker.exists());
    }

    #[test]
    fn a_source_tic_refuses_gives_no_folder() {
        if !tic() {
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        assert_eq!(prepared(cache.path(), "not a terminfo entry"), None);
    }
}
