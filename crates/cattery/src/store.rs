//! The settings file as the Preferences window reads and writes it.

use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use felis_client_core::config::ConfigDocument;
use tempfile::NamedTempFile;
use toml_edit::DocumentMut;

use crate::settings::CLIENT_ID;

/// The settings file at one path.
pub struct Store {
    path: PathBuf,
    /// What this store wrote last, to tell its own writes from others'.
    written: Option<String>,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            written: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file as it is now; an empty one when there is none yet.
    pub fn read(&self) -> anyhow::Result<DocumentMut> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(text.parse()?),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(DocumentMut::new()),
            Err(err) => Err(err.into()),
        }
    }

    /// Replace the file with `doc`, unless felis's reader would refuse it
    /// or the file is a link to one kept elsewhere.
    pub fn write(&mut self, doc: &DocumentMut) -> anyhow::Result<()> {
        let text = doc.to_string();
        let (_, diagnostics) =
            ConfigDocument::parse(&text, Some(self.path.clone()))?.resolve(CLIENT_ID);
        if let Some(error) = diagnostics.errors().next() {
            bail!("felis would refuse these settings: {}", error.message);
        }
        replace(&self.path, &text)?;
        self.written = Some(text);
        Ok(())
    }

    /// Whether the file reads as this store last wrote it.
    pub fn is_as_written(&self) -> bool {
        self.written.as_deref().is_some_and(|written| {
            std::fs::read_to_string(&self.path).is_ok_and(|text| text == written)
        })
    }
}

/// Replace the file at `path` with `text`, unless it is a link to one kept
/// elsewhere.
pub fn replace(path: &Path, text: &str) -> anyhow::Result<()> {
    // Replacing a link would cut it, and whatever keeps the file it points
    // at — a package manager, a dotfiles checkout — would lose track of it.
    if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("{} is kept elsewhere; change it there", path.display());
    }
    let dir = path.parent().context("the file has no directory")?;
    std::fs::create_dir_all(dir)?;
    // Written beside the file and moved over it, so a reader never sees
    // half of it.
    let mut file = NamedTempFile::new_in(dir)?;
    file.write_all(text.as_bytes())?;
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("cattery").join("config.toml"));
        (dir, store)
    }

    fn doc(text: &str) -> DocumentMut {
        text.parse().unwrap()
    }

    #[test]
    fn no_file_reads_as_an_empty_one() {
        let (_dir, store) = store();
        assert_eq!(store.read().unwrap().to_string(), "");
    }

    #[test]
    fn what_is_written_is_read_back_and_the_directory_is_made() {
        let (_dir, mut store) = store();
        store.write(&doc("[font]\nsize_px = 16.0\n")).unwrap();
        assert_eq!(
            store.read().unwrap().to_string(),
            "[font]\nsize_px = 16.0\n"
        );
    }

    #[test]
    fn settings_felis_refuses_are_not_written() {
        let (_dir, mut store) = store();
        store.write(&doc("[font]\nsize_px = 16.0\n")).unwrap();
        assert!(store.write(&doc("[font]\nsize_px = \"large\"\n")).is_err());
        assert_eq!(
            store.read().unwrap().to_string(),
            "[font]\nsize_px = 16.0\n"
        );
    }

    #[test]
    fn a_file_that_does_not_read_as_toml_is_not_taken_for_an_empty_one() {
        let (_dir, store) = store();
        std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        std::fs::write(store.path(), "[font\n").unwrap();
        assert!(store.read().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_linked_from_elsewhere_is_left_to_whatever_keeps_it() {
        let (dir, mut store) = store();
        let kept = dir.path().join("kept.toml");
        std::fs::write(&kept, "[font]\nsize_px = 12.0\n").unwrap();
        std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&kept, store.path()).unwrap();
        assert!(store.write(&doc("[font]\nsize_px = 16.0\n")).is_err());
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "[font]\nsize_px = 12.0\n"
        );
    }

    #[test]
    fn its_own_writes_are_told_from_others() {
        let (_dir, mut store) = store();
        assert!(!store.is_as_written());
        store.write(&doc("[font]\nsize_px = 16.0\n")).unwrap();
        assert!(store.is_as_written());
        std::fs::write(store.path(), "[font]\nsize_px = 20.0\n").unwrap();
        assert!(!store.is_as_written());
    }
}
