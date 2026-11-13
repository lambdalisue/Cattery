//! Which theme colours Cattery: the appearance, and a theme for each of
//! light and dark, kept in a file of Cattery's own beside its felis
//! settings.

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;
use winit::window::Theme;

/// Which of the two themes is in force.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// As the system's appearance is now.
    #[default]
    System,
    Light,
    Dark,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// The word the file spells it with.
    pub const fn word(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appearance {
    pub mode: Mode,
    pub light_theme: String,
    pub dark_theme: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: Mode::System,
            light_theme: "Dawnfox".to_owned(),
            dark_theme: "Nightfox".to_owned(),
        }
    }
}

impl Appearance {
    /// The appearance `text` gives; what it leaves out is the default's.
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: DocumentMut = text.parse().map_err(|err| format!("{err}"))?;
        let mut appearance = Self::default();
        let word = |key: &str| -> Result<Option<String>, String> {
            match doc.get(key) {
                None => Ok(None),
                Some(item) => item
                    .as_str()
                    .map(|word| Some(word.to_owned()))
                    .ok_or_else(|| format!("{key} is not a string")),
            }
        };
        if let Some(mode) = word("appearance")? {
            appearance.mode = Mode::ALL
                .into_iter()
                .find(|each| each.word() == mode)
                .ok_or_else(|| format!("appearance is {mode:?}, not system, light or dark"))?;
        }
        if let Some(theme) = word("light_theme")? {
            appearance.light_theme = theme;
        }
        if let Some(theme) = word("dark_theme")? {
            appearance.dark_theme = theme;
        }
        Ok(appearance)
    }

    /// The name of the theme in force while the system's appearance is
    /// `system`, which some platforms cannot tell.
    pub fn in_force(&self, system: Option<Theme>) -> &str {
        let light = match self.mode {
            Mode::Light => true,
            Mode::Dark => false,
            Mode::System => system == Some(Theme::Light),
        };
        if light {
            &self.light_theme
        } else {
            &self.dark_theme
        }
    }

    /// `text` with this appearance written into it, leaving whatever else
    /// it holds as it is.
    pub fn written_into(&self, text: &str) -> Result<String, String> {
        let mut doc: DocumentMut = text.parse().map_err(|err| format!("{err}"))?;
        for (key, value) in [
            ("appearance", self.mode.word()),
            ("light_theme", self.light_theme.as_str()),
            ("dark_theme", self.dark_theme.as_str()),
        ] {
            match doc.get_mut(key).and_then(toml_edit::Item::as_value_mut) {
                // The comment after a value stays with the value.
                Some(old) => {
                    let decor = old.decor().clone();
                    *old = value.into();
                    *old.decor_mut() = decor;
                }
                None => doc[key] = toml_edit::value(value),
            }
        }
        Ok(doc.to_string())
    }
}

/// The appearance file beside the felis settings file at `settings`.
pub fn path(settings: &Path) -> PathBuf {
    settings.with_file_name("appearance.toml")
}

/// The folder of the user's own themes beside the felis settings file at
/// `settings`.
pub fn themes_dir(settings: &Path) -> PathBuf {
    settings.with_file_name("themes")
}

/// The appearance in the file at `path`; the default when there is none.
pub fn load(path: &Path) -> Result<Appearance, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Appearance::parse(&text).map_err(|err| format!("{}: {err}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Appearance::default()),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_default() {
        assert_eq!(Appearance::parse("").unwrap(), Appearance::default());
    }

    #[test]
    fn the_file_names_the_appearance_and_both_themes() {
        let appearance = Appearance::parse(
            "appearance = \"dark\"\nlight_theme = \"Dayfox\"\ndark_theme = \"Nord\"\n",
        )
        .unwrap();
        assert_eq!(
            appearance,
            Appearance {
                mode: Mode::Dark,
                light_theme: "Dayfox".to_owned(),
                dark_theme: "Nord".to_owned(),
            }
        );
    }

    #[test]
    fn an_appearance_of_no_such_kind_is_refused() {
        assert!(Appearance::parse("appearance = \"sepia\"\n").is_err());
        assert!(Appearance::parse("appearance = 1\n").is_err());
        assert!(Appearance::parse("appearance = \n").is_err());
    }

    #[test]
    fn the_system_s_appearance_picks_the_theme() {
        let appearance = Appearance::default();
        assert_eq!(appearance.in_force(Some(Theme::Light)), "Dawnfox");
        assert_eq!(appearance.in_force(Some(Theme::Dark)), "Nightfox");
    }

    #[test]
    fn a_system_that_cannot_tell_is_taken_for_dark() {
        assert_eq!(Appearance::default().in_force(None), "Nightfox");
    }

    #[test]
    fn a_fixed_appearance_keeps_to_its_theme() {
        let light = Appearance {
            mode: Mode::Light,
            ..Appearance::default()
        };
        assert_eq!(light.in_force(Some(Theme::Dark)), "Dawnfox");
        let dark = Appearance {
            mode: Mode::Dark,
            ..Appearance::default()
        };
        assert_eq!(dark.in_force(Some(Theme::Light)), "Nightfox");
    }

    #[test]
    fn what_is_written_reads_back_and_the_rest_of_the_file_stays() {
        let appearance = Appearance {
            mode: Mode::Light,
            light_theme: "Catppuccin Latte".to_owned(),
            dark_theme: "Nord".to_owned(),
        };
        let text = appearance
            .written_into("# chosen in the Preferences window\nappearance = \"dark\" # was dark\n")
            .unwrap();
        assert!(
            text.starts_with("# chosen in the Preferences window\n"),
            "{text}"
        );
        assert!(text.contains("appearance = \"light\" # was dark"), "{text}");
        assert_eq!(Appearance::parse(&text).unwrap(), appearance);
    }

    #[test]
    fn the_files_sit_beside_the_felis_settings() {
        let settings = Path::new("/home/me/.config/cattery/config.toml");
        assert_eq!(
            path(settings),
            Path::new("/home/me/.config/cattery/appearance.toml")
        );
        assert_eq!(
            themes_dir(settings),
            Path::new("/home/me/.config/cattery/themes")
        );
    }
}
