//! Where Cattery keeps its files on this platform.

use std::path::Path;

use directories::ProjectDirs;

/// Cattery's folders: named Cattery, or cattery where the platform writes
/// an app's folder names in lower case, as Linux does.
pub fn project() -> Option<ProjectDirs> {
    // An empty qualifier and organization, as felis passes: macOS would
    // otherwise join them into a bundle id in the folder's name.
    ProjectDirs::from("", "", "Cattery")
}

/// Write the name of every folder of Cattery's above `dir` as `project`
/// names it, where an earlier Cattery made it under its name in lower case.
pub fn rename_old(dir: &Path) {
    for folder in dir.ancestors() {
        let named_cattery = folder
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("cattery"));
        if named_cattery && let Err(err) = rename_case(folder) {
            tracing::warn!(?err, ?folder, "the folder was not renamed");
        }
    }
}

/// Give the folder `dir` its name as written, where a folder of the same
/// name in other cases stands for it: macOS and Windows do not tell cases
/// apart, so the old folder is found at the new name and kept its old one.
fn rename_case(dir: &Path) -> std::io::Result<()> {
    let (Some(parent), Some(name)) = (dir.parent(), dir.file_name()) else {
        return Ok(());
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Ok(());
    };
    let wanted = name.to_string_lossy();
    for entry in entries.flatten() {
        let found = entry.file_name();
        if found != name && found.to_string_lossy().eq_ignore_ascii_case(&wanted) {
            return std::fs::rename(entry.path(), dir);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn an_old_lower_case_folder_takes_the_new_name_and_keeps_its_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("cattery/themes")).unwrap();
        std::fs::write(root.path().join("cattery/config.toml"), "").unwrap();
        rename_old(&root.path().join("Cattery/config.toml"));
        assert_eq!(names(root.path()), ["Cattery"]);
        assert_eq!(
            names(&root.path().join("Cattery")),
            ["config.toml", "themes"]
        );
    }

    #[test]
    fn a_folder_named_as_wanted_is_left_alone() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Cattery")).unwrap();
        rename_old(&root.path().join("Cattery"));
        assert_eq!(names(root.path()), ["Cattery"]);
    }

    #[test]
    fn no_folder_yet_is_nothing_to_rename() {
        let root = tempfile::tempdir().unwrap();
        rename_old(&root.path().join("Cattery/config.toml"));
        assert!(names(root.path()).is_empty());
    }

    #[test]
    fn the_folders_are_named_cattery() {
        let project = project().unwrap();
        let named = project.config_dir().ancestors().any(|folder| {
            folder
                .file_name()
                .is_some_and(|name| name == "Cattery" || name == "cattery")
        });
        assert!(named, "{:?}", project.config_dir());
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            assert!(
                project
                    .config_dir()
                    .ancestors()
                    .any(|folder| folder.file_name().is_some_and(|name| name == "Cattery")),
                "{:?}",
                project.config_dir()
            );
        }
    }
}
