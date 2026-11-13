//! Cattery's settings: a file in felis's configuration format, read with
//! felis's own reader, and what its windows make of it.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use felis_client_core::config::{EffectiveConfig, FontConfig, FontStyleConfig, LoadError};
use felis_protocol::messages::SessionToDaemonMsg;
use felis_render_core::palette::{self, ThemeConfig};
use felis_render_wgpu::{AxisSetting, DEFAULT_FONT_SIZE_LOGICAL_PX, FaceSpec, RendererConfig};

/// Names the settings file to use instead of the default, for tests and
/// for trying settings out.
pub const OVERRIDE_VAR: &str = "CATTERY_CONFIG";

/// The name `[client.<name>]` sections are written for: settings there
/// apply to Cattery and not to felis's own windows.
pub const CLIENT_ID: &str = "cattery";

/// The settings file: `overridden` when it is set and not empty, otherwise
/// `cattery/config.toml` in the platform's configuration directory. `None`
/// when there is no home directory to find that in.
pub fn path(overridden: Option<OsString>) -> Option<PathBuf> {
    match overridden {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        // An empty qualifier and organization, as felis passes: macOS
        // would otherwise join them into a bundle id in the directory name.
        _ => Some(
            ProjectDirs::from("", "", "cattery")?
                .config_dir()
                .join("config.toml"),
        ),
    }
}

/// The settings in the file at `path`; the defaults when there is no file.
pub fn load(path: &Path) -> Result<EffectiveConfig, LoadError> {
    EffectiveConfig::try_load_from(path, CLIENT_ID)
}

/// The font size, in logical pixels, the settings ask for.
pub fn font_size(settings: &EffectiveConfig) -> f32 {
    settings
        .font
        .sanitized_font_size_logical_px()
        .unwrap_or(DEFAULT_FONT_SIZE_LOGICAL_PX)
}

/// How a window draws with these settings. The font size is left out: it
/// is in physical pixels, which only a window's scale factor gives.
pub fn look(settings: &EffectiveConfig) -> RendererConfig {
    let font = &settings.font;
    RendererConfig {
        font_family: font.family.clone(),
        font_size_physical_px: None,
        cell_scale: Some(font.cell_scale()),
        theme: ThemeConfig {
            fg: settings.theme.foreground.clone(),
            bg: settings.theme.background.clone(),
            palette: settings.theme.palette.clone().into_overrides(),
            cursor: settings.cursor.color.clone(),
            // An opaque window carries no alpha at all, so its surface
            // stays in the opaque mode.
            background_opacity: Some(settings.window.clamped_opacity()).filter(|a| *a < 1.0),
            minimum_contrast: Some(settings.theme.clamped_minimum_contrast())
                .filter(|ratio| *ratio > 1.0),
        },
        font_fallbacks: font.fallback.iter().map(face).collect(),
        font_features: font.features.clone(),
        font_weight: font.weight,
        font_axes: axes(&font.axes),
        font_bold: face(&font.bold),
        font_italic: face(&font.italic),
        font_bold_italic: face(&font.bold_italic),
        // macOS blends text in sRGB already.
        text_alpha_correction: !cfg!(target_os = "macos"),
        ..RendererConfig::default()
    }
}

fn face(style: &FontStyleConfig) -> FaceSpec {
    FaceSpec {
        family: style.family.clone(),
        features: style.features.clone(),
        weight: style.weight,
        axes: axes(&style.axes),
    }
}

fn axes(axes: &BTreeMap<String, f32>) -> Vec<AxisSetting> {
    axes.iter()
        .filter_map(|(tag, &value)| {
            Some(AxisSetting {
                tag: tag.as_bytes().try_into().ok()?,
                value,
            })
        })
        .collect()
}

/// What the daemon answers a program asking for the terminal's colours.
pub fn reported_colors(look: &RendererConfig) -> SessionToDaemonMsg {
    let theme = &look.theme;
    let (fg, bg, cursor) = palette::configured_theme_report(
        theme.fg.as_deref(),
        theme.bg.as_deref(),
        theme.cursor.as_deref(),
    );
    SessionToDaemonMsg::ConfigureTheme {
        fg,
        bg,
        cursor,
        palette: palette::configured_palette_report(&theme.palette),
    }
}

/// A step of a window's text size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    In,
    Out,
    /// Back to the size the settings give.
    Reset,
}

/// The size, in logical pixels, a window drawn at `current` is drawn at
/// after `step`, where the settings give `configured`.
pub fn zoomed(current: f32, step: Zoom, configured: f32) -> f32 {
    match step {
        Zoom::In => FontConfig::clamped_font_size(current, 1.0),
        Zoom::Out => FontConfig::clamped_font_size(current, -1.0),
        Zoom::Reset => configured,
    }
}

/// What a window has to redo when its look changes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The faces themselves differ, and are looked up again.
    pub faces: bool,
    /// Only the size of the same faces differs.
    pub size: bool,
    /// Only the OpenType features applied to the same faces differ.
    pub features: bool,
    /// How cells are scaled from the font's own metrics differs.
    pub cell_scale: bool,
    /// The colours a program can ask the terminal for differ.
    pub reported_colors: bool,
}

impl Changes {
    /// `sizes` are the font sizes in logical pixels before and after, which
    /// a look leaves out.
    pub fn between(before: &RendererConfig, after: &RendererConfig, sizes: (f32, f32)) -> Self {
        let faces = before.font_family != after.font_family
            || before.font_fallbacks != after.font_fallbacks
            || before.font_weight != after.font_weight
            || before.font_axes != after.font_axes
            || before.font_bold != after.font_bold
            || before.font_italic != after.font_italic
            || before.font_bold_italic != after.font_bold_italic;
        let theme = (&before.theme, &after.theme);
        Self {
            faces,
            // Looking the faces up again takes the size and features along.
            size: !faces && (sizes.0 - sizes.1).abs() >= f32::EPSILON,
            features: !faces && before.font_features != after.font_features,
            cell_scale: before.cell_scale != after.cell_scale,
            reported_colors: theme.0.fg != theme.1.fg
                || theme.0.bg != theme.1.bg
                || theme.0.cursor != theme.1.cursor
                || theme.0.palette != theme.1.palette,
        }
    }

    /// Whether the cells change size, so the grid has to be fitted again.
    pub fn refits(&self) -> bool {
        self.faces || self.size || self.cell_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        (dir, path)
    }

    fn settings(text: &str) -> EffectiveConfig {
        let (_dir, path) = written(text);
        load(&path).unwrap()
    }

    #[test]
    fn zooming_in_and_out_steps_a_pixel() {
        assert_eq!(zoomed(14.0, Zoom::In, 14.0), 15.0);
        assert_eq!(zoomed(14.0, Zoom::Out, 14.0), 13.0);
    }

    #[test]
    fn zooming_stays_within_the_sizes_felis_draws() {
        assert_eq!(zoomed(72.0, Zoom::In, 14.0), 72.0);
        assert_eq!(zoomed(4.0, Zoom::Out, 14.0), 4.0);
    }

    #[test]
    fn resetting_returns_to_the_size_the_settings_give() {
        assert_eq!(zoomed(20.0, Zoom::Reset, 13.0), 13.0);
    }

    #[test]
    fn an_override_names_the_file() {
        let got = path(Some(OsString::from("/elsewhere/cattery.toml")));
        assert_eq!(got, Some(PathBuf::from("/elsewhere/cattery.toml")));
    }

    #[test]
    fn an_empty_override_is_no_override() {
        assert_eq!(path(Some(OsString::new())), path(None));
    }

    #[test]
    fn the_file_is_cattery_s_own_not_felis_s() {
        if let Some(path) = path(None) {
            // Windows keeps it a folder deeper, in cattery\config.
            assert_eq!(path.file_name(), Some("config.toml".as_ref()), "{path:?}");
            assert!(
                path.components().any(|part| part.as_os_str() == "cattery"),
                "{path:?}"
            );
            assert!(
                !path.components().any(|part| part.as_os_str() == "felis"),
                "{path:?}"
            );
        }
    }

    #[test]
    fn no_file_means_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let got = load(&dir.path().join("config.toml")).unwrap();
        assert_eq!(font_size(&got), DEFAULT_FONT_SIZE_LOGICAL_PX);
        assert_eq!(got.shell.command, Vec::<String>::new());
    }

    #[test]
    fn the_file_is_read_in_felis_s_format() {
        let got = settings("[font]\nfamily = \"Iosevka\"\nsize_px = 18.0\n");
        assert_eq!(got.font.family.as_deref(), Some("Iosevka"));
        assert_eq!(font_size(&got), 18.0);
    }

    #[test]
    fn a_section_for_cattery_applies() {
        let got = settings("[font]\nsize_px = 13.0\n[client.cattery.font]\nsize_px = 16.0\n");
        assert_eq!(font_size(&got), 16.0);
    }

    #[test]
    fn a_section_for_felis_s_own_windows_does_not() {
        let got = settings("[font]\nsize_px = 13.0\n[client.felis.font]\nsize_px = 16.0\n");
        assert_eq!(font_size(&got), 13.0);
    }

    #[test]
    fn a_file_felis_s_reader_refuses_is_an_error() {
        let (_dir, path) = written("[font\n");
        assert!(load(&path).is_err());
    }

    #[test]
    fn a_size_out_of_range_falls_back_to_the_default() {
        let got = settings("[font]\nsize_px = -3.0\n");
        assert_eq!(font_size(&got), DEFAULT_FONT_SIZE_LOGICAL_PX);
    }

    #[test]
    fn the_look_carries_the_font_and_colors() {
        let look = look(&settings(
            "[font]\nfamily = \"Iosevka\"\n\
             [theme]\nforeground = \"#c0c0c0\"\nbackground = \"#101010\"\n\
             [theme.palette]\nred = \"#ff0000\"\n\
             [cursor]\ncolor = \"#00ff00\"\n",
        ));
        assert_eq!(look.font_family.as_deref(), Some("Iosevka"));
        assert_eq!(look.theme.fg.as_deref(), Some("#c0c0c0"));
        assert_eq!(look.theme.bg.as_deref(), Some("#101010"));
        assert_eq!(look.theme.cursor.as_deref(), Some("#00ff00"));
        assert_eq!(look.theme.palette.len(), 1);
        assert_eq!(look.font_size_physical_px, None);
    }

    #[test]
    fn programs_asking_for_colors_are_told_the_configured_ones() {
        let look = look(&settings(
            "[theme]\nforeground = \"#c0c0c0\"\n[theme.palette]\nred = \"#ff0000\"\n",
        ));
        match reported_colors(&look) {
            SessionToDaemonMsg::ConfigureTheme { fg, palette, .. } => {
                assert_eq!(fg, Some((0xc0, 0xc0, 0xc0)));
                assert_eq!(palette.into_iter().collect::<Vec<_>>(), [(1, (0xff, 0, 0))]);
            }
            other => panic!("not the colors: {other:?}"),
        }
    }

    #[test]
    fn an_opaque_window_carries_no_opacity() {
        assert_eq!(look(&settings("")).theme.background_opacity, None);
        let translucent = look(&settings("[window]\nopacity = 0.8\n"));
        assert_eq!(translucent.theme.background_opacity, Some(0.8));
    }

    fn change(before: &str, after: &str) -> Changes {
        let (before, after) = (settings(before), settings(after));
        Changes::between(
            &look(&before),
            &look(&after),
            (font_size(&before), font_size(&after)),
        )
    }

    #[test]
    fn nothing_changed_needs_nothing_redone() {
        assert_eq!(
            change("[font]\nsize_px = 15.0\n", "[font]\nsize_px = 15.0\n"),
            Changes::default()
        );
    }

    #[test]
    fn another_family_looks_the_faces_up_again() {
        let got = change("", "[font]\nfamily = \"Iosevka\"\n");
        assert!(got.faces);
        assert!(got.refits());
    }

    #[test]
    fn another_size_only_resizes_the_faces() {
        let got = change("", "[font]\nsize_px = 20.0\n");
        assert_eq!(
            got,
            Changes {
                size: true,
                ..Changes::default()
            }
        );
        assert!(got.refits());
    }

    #[test]
    fn other_features_keep_the_faces() {
        let got = change("", "[font]\nfeatures = [\"-liga\"]\n");
        assert_eq!(
            got,
            Changes {
                features: true,
                ..Changes::default()
            }
        );
        assert!(!got.refits());
    }

    #[test]
    fn another_foreground_is_reported_to_programs() {
        let got = change("", "[theme]\nforeground = \"#ffffff\"\n");
        assert_eq!(
            got,
            Changes {
                reported_colors: true,
                ..Changes::default()
            }
        );
    }

    #[test]
    fn another_opacity_is_not_a_color_programs_can_ask_for() {
        assert_eq!(change("", "[window]\nopacity = 0.5\n"), Changes::default());
    }
}
