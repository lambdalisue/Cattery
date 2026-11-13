//! Colour themes in Ghostty's theme format: those Cattery comes with, and
//! the user's own in Cattery's themes folder.

use std::path::Path;

use felis_client_core::config::EffectiveConfig;
use felis_render_core::palette::PALETTE_NAMES;

use crate::edit::{self, Rgb};

/// The themes Cattery comes with, by name. Where they come from and under
/// which licence is in `themes/README.md`.
const SHIPPED: &[(&str, &str)] = &[
    ("Carbonfox", include_str!("../themes/Carbonfox")),
    (
        "Catppuccin Latte",
        include_str!("../themes/Catppuccin Latte"),
    ),
    (
        "Catppuccin Mocha",
        include_str!("../themes/Catppuccin Mocha"),
    ),
    ("Dawnfox", include_str!("../themes/Dawnfox")),
    ("Dayfox", include_str!("../themes/Dayfox")),
    ("Dracula", include_str!("../themes/Dracula")),
    ("Duskfox", include_str!("../themes/Duskfox")),
    (
        "Everforest Light Med",
        include_str!("../themes/Everforest Light Med"),
    ),
    (
        "GitHub Light Default",
        include_str!("../themes/GitHub Light Default"),
    ),
    ("Gruvbox Dark", include_str!("../themes/Gruvbox Dark")),
    ("Gruvbox Light", include_str!("../themes/Gruvbox Light")),
    (
        "Gruvbox Material Dark",
        include_str!("../themes/Gruvbox Material Dark"),
    ),
    (
        "Gruvbox Material Light",
        include_str!("../themes/Gruvbox Material Light"),
    ),
    ("Kanagawa Lotus", include_str!("../themes/Kanagawa Lotus")),
    ("Kanagawa Wave", include_str!("../themes/Kanagawa Wave")),
    ("Melange Light", include_str!("../themes/Melange Light")),
    ("Nightfox", include_str!("../themes/Nightfox")),
    ("Nord", include_str!("../themes/Nord")),
    ("Nordfox", include_str!("../themes/Nordfox")),
    ("One Half Light", include_str!("../themes/One Half Light")),
    ("Rose Pine", include_str!("../themes/Rose Pine")),
    ("Rose Pine Dawn", include_str!("../themes/Rose Pine Dawn")),
    ("Terafox", include_str!("../themes/Terafox")),
    ("TokyoNight", include_str!("../themes/TokyoNight")),
    ("TokyoNight Day", include_str!("../themes/TokyoNight Day")),
    ("Zenbones Light", include_str!("../themes/Zenbones Light")),
];

/// Whether a theme is for a light appearance or a dark one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Light,
    Dark,
}

/// A theme's colours. A colour it leaves out is felis's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Colors {
    pub foreground: Option<Rgb>,
    pub background: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub palette: [Option<Rgb>; 16],
}

impl Colors {
    /// The colours `text` gives, in Ghostty's theme format.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut colors = Self::default();
        for (at, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = || format!("line {}: {line:?} is not a colour", at + 1);
            let (key, value) = line.split_once('=').ok_or_else(bad)?;
            let color = |value: &str| edit::rgb(value.trim()).ok_or_else(bad);
            match key.trim() {
                "palette" => {
                    let (index, value) = value.split_once('=').ok_or_else(bad)?;
                    let index: usize = index.trim().parse().map_err(|_| bad())?;
                    *colors.palette.get_mut(index).ok_or_else(bad)? = Some(color(value)?);
                }
                "background" => colors.background = Some(color(value)?),
                "foreground" => colors.foreground = Some(color(value)?),
                "cursor-color" => colors.cursor = Some(color(value)?),
                // Ghostty's other keys colour what felis has no setting for.
                _ => {}
            }
        }
        Ok(colors)
    }

    /// Whether the theme is for a dark appearance: its background darker
    /// than its text.
    pub fn kind(&self) -> Kind {
        let luminance = |rgb: Rgb| {
            let [r, g, b] = rgb.map(f64::from);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        let background = self.background.map_or(0.0, luminance);
        let foreground = self.foreground.map_or(255.0, luminance);
        if background < foreground {
            Kind::Dark
        } else {
            Kind::Light
        }
    }

    /// `settings` with every colour they leave unset taken from the theme.
    pub fn under(&self, settings: &EffectiveConfig) -> EffectiveConfig {
        let mut colored = settings.clone();
        let fill = |slot: &mut Option<String>, color: Option<Rgb>| {
            if slot.is_none() {
                *slot = color.map(edit::hex);
            }
        };
        fill(&mut colored.theme.foreground, self.foreground);
        fill(&mut colored.theme.background, self.background);
        fill(&mut colored.cursor.color, self.cursor);
        let palette = &mut colored.theme.palette;
        let slots = [
            &mut palette.black,
            &mut palette.red,
            &mut palette.green,
            &mut palette.yellow,
            &mut palette.blue,
            &mut palette.magenta,
            &mut palette.cyan,
            &mut palette.white,
            &mut palette.bright_black,
            &mut palette.bright_red,
            &mut palette.bright_green,
            &mut palette.bright_yellow,
            &mut palette.bright_blue,
            &mut palette.bright_magenta,
            &mut palette.bright_cyan,
            &mut palette.bright_white,
        ];
        debug_assert_eq!(slots.len(), PALETTE_NAMES.len());
        for (slot, color) in slots.into_iter().zip(self.palette) {
            fill(slot, color);
        }
        colored
    }
}

/// Every theme to choose from: Cattery's, then the user's in `dir` by name.
pub fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = SHIPPED.iter().map(|(name, _)| (*name).to_owned()).collect();
    let mut own: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && !names.contains(name))
        .collect();
    own.sort();
    names.extend(own);
    names
}

/// The colours of the theme `name`: the user's file of that name in `dir`
/// first, so one of Cattery's can be changed by writing it there.
pub fn colors(name: &str, dir: &Path) -> Result<Colors, String> {
    let file = dir.join(name);
    if !name.contains(['/', '\\']) && file.is_file() {
        let text = std::fs::read_to_string(&file).map_err(|err| err.to_string())?;
        return Colors::parse(&text).map_err(|err| format!("{}: {err}", file.display()));
    }
    let (_, text) = SHIPPED
        .iter()
        .find(|(shipped, _)| *shipped == name)
        .ok_or_else(|| format!("no theme named {name:?}"))?;
    Colors::parse(text)
}

/// Every theme to choose from, as [`names`] lists them, with its kind.
pub fn listed(dir: &Path) -> Vec<(String, Kind)> {
    names(dir)
        .into_iter()
        .map(|name| {
            // A theme that cannot be read is offered beside the dark ones.
            let kind = colors(&name, dir).map_or(Kind::Dark, |colors| colors.kind());
            (name, kind)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use felis_client_core::config::ConfigDocument;

    use super::*;
    use crate::settings::CLIENT_ID;

    const NIGHT: &str = "# a comment\n\
        palette = 0=#393b44\n\
        palette = 9=#d16983\n\
        background = #192330\n\
        foreground = #cdcecf\n\
        cursor-color = #aeafb0\n\
        selection-background = #2b3b51\n";

    fn none() -> &'static Path {
        Path::new("/no/such/folder")
    }

    #[test]
    fn a_theme_gives_its_colours() {
        let colors = Colors::parse(NIGHT).unwrap();
        assert_eq!(colors.background, Some([0x19, 0x23, 0x30]));
        assert_eq!(colors.foreground, Some([0xcd, 0xce, 0xcf]));
        assert_eq!(colors.cursor, Some([0xae, 0xaf, 0xb0]));
        assert_eq!(colors.palette[0], Some([0x39, 0x3b, 0x44]));
        assert_eq!(colors.palette[9], Some([0xd1, 0x69, 0x83]));
        assert_eq!(colors.palette[1], None);
    }

    #[test]
    fn a_line_that_is_not_a_colour_is_refused() {
        assert!(Colors::parse("background = dark\n").is_err());
        assert!(Colors::parse("palette = 16=#000000\n").is_err());
        assert!(Colors::parse("just words\n").is_err());
    }

    #[test]
    fn a_dark_background_under_light_text_is_a_dark_theme() {
        assert_eq!(Colors::parse(NIGHT).unwrap().kind(), Kind::Dark);
        let day = "background = #f6f2ee\nforeground = #3d2b5a\n";
        assert_eq!(Colors::parse(day).unwrap().kind(), Kind::Light);
    }

    #[test]
    fn the_theme_colours_what_the_settings_leave_unset() {
        let (settings, _) = ConfigDocument::parse("[theme]\nforeground = \"#ffffff\"\n", None)
            .unwrap()
            .resolve(CLIENT_ID);
        let colored = Colors::parse(NIGHT).unwrap().under(&settings);
        assert_eq!(colored.theme.foreground.as_deref(), Some("#ffffff"));
        assert_eq!(colored.theme.background.as_deref(), Some("#192330"));
        assert_eq!(colored.cursor.color.as_deref(), Some("#aeafb0"));
        assert_eq!(colored.theme.palette.bright_red.as_deref(), Some("#d16983"));
        assert_eq!(colored.theme.palette.red, None);
    }

    #[test]
    fn every_shipped_theme_reads() {
        for (name, text) in SHIPPED {
            assert!(Colors::parse(text).is_ok(), "{name}");
        }
    }

    #[test]
    fn as_many_light_themes_are_shipped_as_dark() {
        let kinds: Vec<Kind> = listed(none()).into_iter().map(|(_, kind)| kind).collect();
        let light = kinds.iter().filter(|kind| **kind == Kind::Light).count();
        assert_eq!(light * 2, kinds.len());
    }

    #[test]
    fn the_user_s_themes_follow_cattery_s() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Mine"), "background = #010203\n").unwrap();
        std::fs::write(dir.path().join(".hidden"), "background = #010203\n").unwrap();
        let names = names(dir.path());
        assert_eq!(names.last().map(String::as_str), Some("Mine"));
        assert!(!names.iter().any(|name| name == ".hidden"));
        assert_eq!(names.len(), SHIPPED.len() + 1);
    }

    #[test]
    fn a_user_s_theme_takes_the_place_of_cattery_s_of_its_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Nightfox"), "background = #010203\n").unwrap();
        let colors = colors("Nightfox", dir.path()).unwrap();
        assert_eq!(colors.background, Some([1, 2, 3]));
        assert_eq!(names(dir.path()).len(), SHIPPED.len());
    }

    #[test]
    fn a_theme_of_no_such_name_is_an_error() {
        assert!(colors("Nothing", none()).is_err());
    }
}
