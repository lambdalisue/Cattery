//! Which theme colours Cattery: the appearance, and a theme for each of
//! light and dark, kept in a file of Cattery's own beside its felis
//! settings.

use std::path::{Path, PathBuf};

use felis_client_core::config::EffectiveConfig;
use toml_edit::{DocumentMut, Value};
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

/// The largest blur radius, in points, the file sets.
pub const MAX_BLUR: u32 = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub mode: Mode,
    pub light_theme: String,
    pub dark_theme: String,
    /// How much of the window's background is drawn, from `0.0` to `1.0`.
    pub opacity: f32,
    /// The radius, in points, of the blur of what shows through a
    /// translucent window; `0` for none.
    pub blur: u32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: Mode::System,
            light_theme: "Dawnfox".to_owned(),
            dark_theme: "Nightfox".to_owned(),
            opacity: 1.0,
            blur: 0,
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
        if let Some(item) = doc.get("opacity") {
            let opacity = item
                .as_float()
                .or_else(|| item.as_integer().map(|whole| whole as f64))
                .ok_or("opacity is not a number")?;
            #[expect(clippy::cast_possible_truncation, reason = "brought into 0..=1")]
            let opacity = opacity.clamp(0.0, 1.0) as f32;
            appearance.opacity = opacity;
        }
        if let Some(item) = doc.get("blur") {
            let blur = item.as_integer().ok_or("blur is not a whole number")?;
            appearance.blur = u32::try_from(blur.clamp(0, i64::from(MAX_BLUR))).unwrap_or(0);
        }
        Ok(appearance)
    }

    /// Draw the window in `drawn` as opaque as this appearance says, in
    /// place of what felis's settings say.
    pub fn set_window(&self, drawn: &mut EffectiveConfig) {
        drawn.window.opacity = self.opacity;
    }

    /// Whether Cattery looks light or dark while the system's appearance
    /// is `system`, which some platforms cannot tell.
    pub fn look(&self, system: Option<Theme>) -> Theme {
        match self.mode {
            Mode::Light => Theme::Light,
            Mode::Dark => Theme::Dark,
            Mode::System => system.unwrap_or(Theme::Dark),
        }
    }

    /// The look a window's frame — its title, its buttons — is told to take,
    /// or none to leave it to the system's.
    pub fn frame(&self) -> Option<Theme> {
        match self.mode {
            Mode::Light => Some(Theme::Light),
            Mode::Dark => Some(Theme::Dark),
            Mode::System => None,
        }
    }

    /// The name of the theme in force while the system's appearance is
    /// `system`.
    pub fn in_force(&self, system: Option<Theme>) -> &str {
        match self.look(system) {
            Theme::Light => &self.light_theme,
            Theme::Dark => &self.dark_theme,
        }
    }

    /// `text` with this appearance written into it, leaving whatever else
    /// it holds as it is.
    pub fn written_into(&self, text: &str) -> Result<String, String> {
        let mut doc: DocumentMut = text.parse().map_err(|err| format!("{err}"))?;
        // The shortest decimal that reads back as the same f32, so the file
        // says 0.8 and not the f64 nearest to it.
        let opacity = self
            .opacity
            .to_string()
            .parse::<f64>()
            .unwrap_or(f64::from(self.opacity));
        for (key, value) in [
            ("appearance", Value::from(self.mode.word())),
            ("light_theme", Value::from(self.light_theme.as_str())),
            ("dark_theme", Value::from(self.dark_theme.as_str())),
            ("opacity", Value::from(opacity)),
            ("blur", Value::from(i64::from(self.blur))),
        ] {
            match doc.get_mut(key).and_then(toml_edit::Item::as_value_mut) {
                // The comment after a value stays with the value.
                Some(old) => {
                    let decor = old.decor().clone();
                    *old = value;
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
                ..Appearance::default()
            }
        );
    }

    #[test]
    fn a_window_is_opaque_and_unblurred_by_default() {
        let appearance = Appearance::default();
        assert_eq!((appearance.opacity, appearance.blur), (1.0, 0));
    }

    #[test]
    fn the_file_names_the_window_s_opacity_and_blur() {
        let appearance = Appearance::parse("opacity = 0.8\nblur = 20\n").unwrap();
        assert_eq!((appearance.opacity, appearance.blur), (0.8, 20));
    }

    #[test]
    fn the_window_is_drawn_as_opaque_as_the_appearance_says_whatever_felis_s_settings_say() {
        let (settings, _) =
            felis_client_core::config::ConfigDocument::parse("[window]\nopacity = 0.5\n", None)
                .unwrap()
                .resolve(crate::settings::CLIENT_ID);
        let mut drawn = settings;
        Appearance::default().set_window(&mut drawn);
        assert_eq!(drawn.window.opacity, 1.0);
        let translucent = Appearance {
            opacity: 0.7,
            ..Appearance::default()
        };
        translucent.set_window(&mut drawn);
        assert_eq!(drawn.window.opacity, 0.7);
    }

    #[test]
    fn a_whole_opacity_is_a_number_too() {
        assert_eq!(Appearance::parse("opacity = 1\n").unwrap().opacity, 1.0);
    }

    #[test]
    fn an_opacity_or_blur_out_of_range_is_brought_into_it() {
        let appearance = Appearance::parse("opacity = 1.5\nblur = 1000\n").unwrap();
        assert_eq!((appearance.opacity, appearance.blur), (1.0, MAX_BLUR));
        let appearance = Appearance::parse("opacity = -0.5\nblur = -3\n").unwrap();
        assert_eq!((appearance.opacity, appearance.blur), (0.0, 0));
    }

    #[test]
    fn an_opacity_or_blur_that_is_not_a_number_is_refused() {
        assert!(Appearance::parse("opacity = \"half\"\n").is_err());
        assert!(Appearance::parse("blur = 2.5\n").is_err());
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
    fn a_light_theme_in_force_makes_the_look_light() {
        let appearance = Appearance::default();
        assert_eq!(appearance.look(Some(Theme::Light)), Theme::Light);
        assert_eq!(appearance.look(Some(Theme::Dark)), Theme::Dark);
        assert_eq!(appearance.look(None), Theme::Dark);
        let light = Appearance {
            mode: Mode::Light,
            ..Appearance::default()
        };
        assert_eq!(light.look(Some(Theme::Dark)), Theme::Light);
    }

    #[test]
    fn a_fixed_appearance_sets_the_window_s_frame_and_the_system_s_leaves_it_to_the_system() {
        assert_eq!(Appearance::default().frame(), None);
        let light = Appearance {
            mode: Mode::Light,
            ..Appearance::default()
        };
        assert_eq!(light.frame(), Some(Theme::Light));
        let dark = Appearance {
            mode: Mode::Dark,
            ..Appearance::default()
        };
        assert_eq!(dark.frame(), Some(Theme::Dark));
    }

    #[test]
    fn what_is_written_reads_back_and_the_rest_of_the_file_stays() {
        let appearance = Appearance {
            mode: Mode::Light,
            light_theme: "Catppuccin Latte".to_owned(),
            dark_theme: "Nord".to_owned(),
            opacity: 0.85,
            blur: 12,
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
