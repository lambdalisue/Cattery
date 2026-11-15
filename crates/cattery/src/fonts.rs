//! The font Cattery ships, searched before the system's fonts; which
//! licence it is under is in `fonts/README.md`.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The family of the faces Cattery ships.
pub const FAMILY: &str = "JetBrainsMono Nerd Font Mono";

const FACES: &[(&str, &[u8])] = &[
    (
        "JetBrainsMonoNerdFontMono-Regular.ttf",
        include_bytes!("../fonts/JetBrainsMonoNerdFontMono-Regular.ttf"),
    ),
    (
        "JetBrainsMonoNerdFontMono-Bold.ttf",
        include_bytes!("../fonts/JetBrainsMonoNerdFontMono-Bold.ttf"),
    ),
    (
        "JetBrainsMonoNerdFontMono-Italic.ttf",
        include_bytes!("../fonts/JetBrainsMonoNerdFontMono-Italic.ttf"),
    ),
    (
        "JetBrainsMonoNerdFontMono-BoldItalic.ttf",
        include_bytes!("../fonts/JetBrainsMonoNerdFontMono-BoldItalic.ttf"),
    ),
];

/// The faces Cattery ships, as files felis can read them from: written once
/// a run into Cattery's cache folder, since felis takes fonts as paths. None
/// where they cannot be written, which leaves the system's fonts.
pub fn files() -> Vec<PathBuf> {
    static FILES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    FILES
        .get_or_init(|| {
            let Some(folders) = crate::folders::project() else {
                tracing::warn!("no cache folder to write the bundled font into");
                return Vec::new();
            };
            write(&folders.cache_dir().join("fonts")).unwrap_or_else(|err| {
                tracing::warn!(?err, "the bundled font was not written");
                Vec::new()
            })
        })
        .clone()
}

/// Write the faces Cattery ships into `dir`, and name the files they are
/// in. A file that already holds its face is left alone; any other is
/// replaced whole, so another Cattery reading it never sees part of one.
pub fn write(dir: &Path) -> io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    FACES
        .iter()
        .map(|(name, data)| {
            let path = dir.join(name);
            if std::fs::read(&path).ok().as_deref() != Some(*data) {
                let mut file = tempfile::NamedTempFile::new_in(dir)?;
                file.write_all(data)?;
                file.persist(&path).map_err(|err| err.error)?;
            }
            Ok(path)
        })
        .collect()
}

/// The families a grid can be set in, for a list to pick from: the one
/// Cattery ships, then the `installed` ones.
pub fn with_installed(installed: Vec<String>) -> Vec<String> {
    std::iter::once(FAMILY.to_owned())
        .chain(installed.into_iter().filter(|family| family != FAMILY))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    /// The fonts folder in the source, whose licences ship with Cattery.
    fn source() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts")
    }

    #[test]
    fn every_licence_shipped_is_one_the_attributions_name() {
        let attributions = std::fs::read_to_string(source().join("README.md")).unwrap();
        for entry in std::fs::read_dir(source().join("LICENSES")).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                attributions.contains(&format!("`{name}`")),
                "{name} is not named"
            );
        }
    }

    #[test]
    fn every_licence_the_attributions_name_is_shipped() {
        let attributions = std::fs::read_to_string(source().join("README.md")).unwrap();
        let named = attributions
            .lines()
            .filter(|line| line.starts_with("| ") && !line.starts_with("| Work"))
            .filter_map(|line| line.trim_end_matches(" |").rsplit(" | ").next())
            .flat_map(|texts| texts.split(", "))
            .map(|text| text.trim_matches('`'))
            .filter(|text| !text.is_empty() && !text.starts_with("---"));
        for name in named {
            assert!(
                source().join("LICENSES").join(name).is_file(),
                "{name} is not shipped"
            );
        }
    }

    #[test]
    fn the_family_shipped_comes_first_and_once_among_the_installed() {
        let installed = vec!["Menlo".to_owned(), FAMILY.to_owned(), "Iosevka".to_owned()];
        assert_eq!(with_installed(installed), [FAMILY, "Menlo", "Iosevka"]);
    }

    #[test]
    fn the_family_shipped_is_listed_where_none_is_installed() {
        assert_eq!(with_installed(Vec::new()), [FAMILY]);
    }

    #[test]
    fn the_faces_shipped_are_one_fixed_width_family_in_four_styles() {
        let mut fonts = fontdb::Database::new();
        for (_, data) in FACES {
            fonts.load_font_data(data.to_vec());
        }
        let mut styles: Vec<_> = fonts
            .faces()
            .map(|face| {
                assert_eq!(face.families[0].0, FAMILY);
                assert!(
                    face.monospaced,
                    "{} is not fixed-width",
                    face.post_script_name
                );
                (face.weight.0, face.style == fontdb::Style::Italic)
            })
            .collect();
        styles.sort_unstable();
        assert_eq!(
            styles,
            [(400, false), (400, true), (700, false), (700, true)]
        );
    }

    #[test]
    fn every_face_is_written_into_the_folder_whole() {
        let dir = tempfile::tempdir().unwrap();
        let written = write(&dir.path().join("fonts")).unwrap();
        assert_eq!(written.len(), FACES.len());
        for (path, (name, data)) in written.iter().zip(FACES) {
            assert_eq!(path, &dir.path().join("fonts").join(name));
            assert_eq!(&std::fs::read(path).unwrap(), data);
        }
    }

    #[test]
    fn a_face_already_written_is_left_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path()).unwrap().remove(0);
        let long_ago = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
        write(dir.path()).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            long_ago
        );
    }

    #[test]
    fn a_face_that_differs_is_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path()).unwrap().remove(0);
        std::fs::write(&path, b"not a font").unwrap();
        write(dir.path()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), FACES[0].1);
    }
}
