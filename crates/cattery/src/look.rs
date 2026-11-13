//! How Cattery's own windows look — Preferences and Sessions: the platform's own fonts, and colours
//! after the settings windows of macOS, light or dark as the system is.

use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions, FontFamily};
use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Theme, Visuals};

/// The colours every part of the window is drawn in.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub window: Color32,
    pub sidebar: Color32,
    pub group: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub separator: Color32,
    pub border: Color32,
    pub control: Color32,
    pub control_border: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub error: Color32,
}

impl Tokens {
    pub const fn of(dark: bool) -> Self {
        if dark {
            Self {
                window: Color32::from_rgb(0x1e, 0x1e, 0x1e),
                sidebar: Color32::from_rgb(0x26, 0x26, 0x26),
                group: Color32::from_rgb(0x2c, 0x2c, 0x2c),
                text: Color32::from_rgb(0xe5, 0xe5, 0xe7),
                muted: Color32::from_rgb(0x98, 0x98, 0x9d),
                separator: Color32::from_rgb(0x3a, 0x3a, 0x3a),
                border: Color32::from_rgb(0x36, 0x36, 0x36),
                control: Color32::from_rgb(0x3a, 0x3a, 0x3c),
                control_border: Color32::from_rgb(0x4c, 0x4c, 0x4e),
                accent: Color32::from_rgb(0x0a, 0x84, 0xff),
                on_accent: Color32::WHITE,
                error: Color32::from_rgb(0xff, 0x69, 0x61),
            }
        } else {
            Self {
                window: Color32::from_rgb(0xec, 0xec, 0xec),
                sidebar: Color32::from_rgb(0xe3, 0xe3, 0xe3),
                group: Color32::WHITE,
                text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
                muted: Color32::from_rgb(0x86, 0x86, 0x8b),
                separator: Color32::from_rgb(0xe6, 0xe6, 0xe6),
                border: Color32::from_rgb(0xdc, 0xdc, 0xdc),
                control: Color32::WHITE,
                control_border: Color32::from_rgb(0xc8, 0xc8, 0xc8),
                accent: Color32::from_rgb(0x00, 0x7a, 0xff),
                on_accent: Color32::WHITE,
                error: Color32::from_rgb(0xd7, 0x00, 0x15),
            }
        }
    }
}

/// The families the window's text is set in, best first. The platforms'
/// interface faces come first; egui's own faces are left for the rest.
const TEXT: &[&str] = &[
    "SF Pro",
    "SF Pro Text",
    "System Font",
    "Helvetica Neue",
    "Segoe UI",
    "Noto Sans",
    "Cantarell",
    "DejaVu Sans",
];
const JAPANESE: &[&str] = &[
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Yu Gothic UI",
    "Meiryo",
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "IPAexGothic",
];
const CODE: &[&str] = &[
    "SF Mono",
    "Menlo",
    "Consolas",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
];

/// The first of `wanted` that `installed` says is there.
pub fn first_installed<'a>(
    wanted: &[&'a str],
    installed: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    wanted.iter().copied().find(|family| installed(family))
}

/// The family of the bold faces, for titles and headings.
const BOLD: &str = "bold";

/// Text set in the platform's bold interface face.
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// Set `context` up to look as the window should, in either theme.
pub fn install(context: &egui::Context) {
    context.set_fonts(fonts());
    for theme in [Theme::Light, Theme::Dark] {
        context.set_visuals_of(theme, visuals(Tokens::of(theme == Theme::Dark)));
        context.style_mut_of(theme, |style| {
            // The sizes macOS's own settings are set in.
            style.text_styles = [
                (TextStyle::Heading, bold(17.0)),
                (TextStyle::Body, FontId::proportional(13.0)),
                (TextStyle::Button, FontId::proportional(13.0)),
                (TextStyle::Small, FontId::proportional(11.0)),
                (TextStyle::Monospace, FontId::monospace(12.0)),
            ]
            .into();
            style.spacing.item_spacing = egui::vec2(8.0, 4.0);
            style.spacing.button_padding = egui::vec2(10.0, 3.0);
            style.spacing.interact_size = egui::vec2(36.0, 22.0);
            style.spacing.combo_width = 200.0;
            style.spacing.slider_width = 150.0;
            style.spacing.slider_rail_height = 4.0;
            style.spacing.menu_margin = egui::Margin::same(5);
            style.visuals.handle_shape = egui::style::HandleShape::Circle;
        });
    }
}

/// The platform's faces first, then egui's own for whatever they lack.
fn fonts() -> FontDefinitions {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut fonts = FontDefinitions::default();
    let mut put = |name: &str, wanted: &[&str], weight: fontdb::Weight, families: &[FontFamily]| {
        let Some(data) = first_installed(wanted, |family| query(&db, family, weight).is_some())
            .and_then(|family| face(&db, family, weight))
        else {
            return;
        };
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
        for family in families {
            let list = fonts.families.entry(family.clone()).or_default();
            let at = list
                .iter()
                .take_while(|face| face.starts_with("system-"))
                .count();
            list.insert(at, name.to_owned());
        }
    };
    let (regular, semibold) = (fontdb::Weight::NORMAL, fontdb::Weight::SEMIBOLD);
    put("system-text", TEXT, regular, &[FontFamily::Proportional]);
    put("system-code", CODE, regular, &[FontFamily::Monospace]);
    put(
        "system-japanese",
        JAPANESE,
        regular,
        &[FontFamily::Proportional, FontFamily::Monospace],
    );
    let bold = FontFamily::Name(BOLD.into());
    put(
        "system-text-bold",
        TEXT,
        semibold,
        std::slice::from_ref(&bold),
    );
    put(
        "system-japanese-bold",
        JAPANESE,
        semibold,
        std::slice::from_ref(&bold),
    );
    // Whatever the bold faces lack is drawn as the regular text is.
    let regular_faces = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .entry(bold)
        .or_default()
        .extend(regular_faces);
    fonts
}

fn query(db: &fontdb::Database, family: &str, weight: fontdb::Weight) -> Option<fontdb::ID> {
    db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight,
        ..fontdb::Query::default()
    })
}

fn face(db: &fontdb::Database, family: &str, weight: fontdb::Weight) -> Option<FontData> {
    db.with_face_data(query(db, family, weight)?, |data, index| {
        let mut face = FontData::from_owned(data.to_vec());
        face.index = index;
        face
    })
}

fn visuals(tokens: Tokens) -> Visuals {
    let dark = tokens.window.r() < 0x80;
    let base = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(6);
    let control = |fill: Color32| WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, tokens.control_border),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, tokens.text),
        expansion: 0.0,
    };
    Visuals {
        dark_mode: dark,
        override_text_color: Some(tokens.text),
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_stroke: Stroke::new(1.0, tokens.separator),
                ..control(tokens.group)
            },
            inactive: control(tokens.control),
            hovered: control(tokens.control),
            active: control(tokens.control.gamma_multiply(0.85)),
            open: control(tokens.control),
        },
        selection: Selection {
            bg_fill: tokens.accent,
            stroke: Stroke::new(1.0, tokens.on_accent),
        },
        hyperlink_color: tokens.accent,
        faint_bg_color: tokens.group,
        extreme_bg_color: tokens.control,
        window_fill: tokens.group,
        window_stroke: Stroke::new(1.0, tokens.control_border),
        panel_fill: tokens.window,
        error_fg_color: tokens.error,
        slider_trailing_fill: true,
        menu_corner_radius: CornerRadius::same(8),
        window_corner_radius: CornerRadius::same(10),
        popup_shadow: egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 120 } else { 40 }),
        },
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_installed_family_is_taken() {
        let installed = |family: &str| matches!(family, "Segoe UI" | "Noto Sans");
        assert_eq!(first_installed(TEXT, installed), Some("Segoe UI"));
    }

    #[test]
    fn with_none_installed_egui_s_own_faces_are_left() {
        assert_eq!(first_installed(TEXT, |_| false), None);
    }

    #[test]
    fn text_stands_out_from_the_groups_it_sits_in() {
        let luminance = |c: Color32| u16::from(c.r()) + u16::from(c.g()) + u16::from(c.b());
        for dark in [false, true] {
            let tokens = Tokens::of(dark);
            let gap = luminance(tokens.text).abs_diff(luminance(tokens.group));
            assert!(gap > 300, "dark: {dark}");
        }
    }
}
