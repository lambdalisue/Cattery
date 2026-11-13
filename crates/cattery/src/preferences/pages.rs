//! The Preferences window's pages: a sidebar to pick one, and the page as
//! groups of rows, each a setting's name on the left and its control on the
//! right, as macOS's own settings are laid out.

use egui::{
    Align, CentralPanel, ComboBox, CornerRadius, Frame, Layout, Margin, Panel, RichText,
    ScrollArea, Sense, Slider, Stroke, TextEdit, Ui, vec2,
};
use felis_client_core::config::{CursorBlinkMode, EffectiveConfig};
use felis_render_core::palette::{DEFAULT_BG_SRGB, DEFAULT_FG_SRGB, PALETTE_NAMES, XTERM_PALETTE};
use felis_render_wgpu::DEFAULT_FONT_SIZE_LOGICAL_PX;

use super::look::Tokens;
use crate::edit::{self, Color, Edit, Rgb};
use crate::settings;

/// The font sizes, in logical pixels, felis accepts.
const FONT_SIZES: std::ops::RangeInclusive<f32> = 4.0..=72.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Font,
    Colors,
    Cursor,
    Shell,
}

impl Page {
    const ALL: [Self; 4] = [Self::Font, Self::Colors, Self::Cursor, Self::Shell];

    const fn title(self) -> &'static str {
        match self {
            Self::Font => "Font",
            Self::Colors => "Colors",
            Self::Cursor => "Cursor",
            Self::Shell => "Shell",
        }
    }
}

/// What the pages show and where they keep what is being typed.
pub struct View<'a> {
    pub settings: &'a EffectiveConfig,
    /// Installed families felis can draw a grid with.
    pub families: &'a [String],
    /// The shell line as it is being typed.
    pub shell: &'a mut String,
    /// Why the last change was not saved.
    pub problem: Option<&'a str>,
    pub path: &'a str,
}

/// Draw the window on `page`; what the user changed in this frame.
pub fn show(ui: &mut Ui, page: &mut Page, view: View<'_>) -> Vec<Edit> {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    Panel::left("pages")
        .exact_size(200.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(tokens.sidebar)
                .stroke(Stroke::new(1.0, tokens.separator))
                .inner_margin(Margin::symmetric(10, 12)),
        )
        .show(ui, |ui| sidebar(ui, page, tokens));
    let mut edits = Vec::new();
    CentralPanel::default()
        .frame(Frame::new().fill(tokens.window))
        .show(ui, |ui| {
            title(ui, page.title(), tokens);
            ScrollArea::vertical().show(ui, |ui| {
                Frame::new()
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 20,
                        bottom: 32,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let content = &mut Content {
                            ui,
                            tokens,
                            edits: &mut edits,
                        };
                        match page {
                            Page::Font => font(content, &view),
                            Page::Colors => colors(content, &view),
                            Page::Cursor => cursor(content, &view),
                            Page::Shell => shell(content, view.shell),
                        }
                        footer(content, &view);
                    });
            });
        });
    edits
}

fn sidebar(ui: &mut Ui, page: &mut Page, tokens: Tokens) {
    ui.add_space(4.0);
    for item in Page::ALL {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        let selected = *page == item;
        if selected {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), tokens.accent);
        } else if response.hovered() {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), tokens.separator);
        }
        let color = if selected {
            tokens.on_accent
        } else {
            tokens.text
        };
        ui.painter().text(
            rect.left_center() + vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            item.title(),
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
        if response.clicked() {
            *page = item;
        }
        ui.add_space(1.0);
    }
}

fn title(ui: &mut Ui, title: &str, tokens: Tokens) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(24.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(20.0),
        tokens.text,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, tokens.separator),
    );
}

/// A page being drawn, and what the user changed on it.
struct Content<'u, 'e> {
    ui: &'u mut Ui,
    tokens: Tokens,
    edits: &'e mut Vec<Edit>,
}

impl Content<'_, '_> {
    /// A group of rows under `heading`, with `note` saying what they are.
    fn group(
        &mut self,
        heading: Option<&str>,
        note: Option<&str>,
        rows: impl FnOnce(&mut Rows<'_>),
    ) {
        let tokens = self.tokens;
        if let Some(heading) = heading {
            self.ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(heading).strong().size(15.0));
            });
        }
        if let Some(note) = note {
            self.ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(note).small().color(tokens.muted));
            });
        }
        self.ui.add_space(2.0);
        let edits = &mut *self.edits;
        Frame::new()
            .fill(tokens.group)
            .stroke(Stroke::new(1.0, tokens.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(16, 0))
            .show(self.ui, |ui| {
                ui.set_width(ui.available_width());
                rows(&mut Rows {
                    ui,
                    tokens,
                    edits,
                    first: true,
                });
            });
        self.ui.add_space(22.0);
    }
}

/// The rows of one group.
struct Rows<'u> {
    ui: &'u mut Ui,
    tokens: Tokens,
    edits: &'u mut Vec<Edit>,
    first: bool,
}

impl Rows<'_> {
    /// A row naming a setting, with what it is under the name, its control
    /// on the right, and — while it is off its default — a way back to it.
    fn row(
        &mut self,
        name: &str,
        description: Option<&str>,
        control: impl FnOnce(&mut Ui, &mut Vec<Edit>),
        reset: Option<(String, Vec<Edit>)>,
    ) {
        let tokens = self.tokens;
        if !self.first {
            let width = self.ui.available_width();
            let (rect, _) = self
                .ui
                .allocate_exact_size(vec2(width, 1.0), Sense::hover());
            self.ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke::new(1.0, tokens.separator),
            );
        }
        self.first = false;
        self.ui.add_space(10.0);
        let edits = &mut *self.edits;
        self.ui.horizontal(|ui| {
            ui.set_min_height(28.0);
            // The name and what it is take up to half the row, wrapping
            // there rather than running under the control.
            let half = ui.available_width() / 2.0;
            ui.vertical(|ui| {
                ui.set_max_width(half);
                ui.add_space(3.0);
                ui.label(name);
                if let Some(description) = description {
                    ui.add(
                        egui::Label::new(RichText::new(description).small().color(tokens.muted))
                            .wrap(),
                    );
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                control(ui, edits)
            });
        });
        if let Some((label, reset)) = reset {
            self.ui
                .with_layout(Layout::right_to_left(Align::Min), |ui| {
                    let link = ui.add(
                        egui::Label::new(RichText::new(label).small().color(tokens.accent))
                            .sense(Sense::click()),
                    );
                    if link
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        edits.extend(reset);
                    }
                });
        }
        self.ui.add_space(10.0);
    }

    /// A row picking `which` colour, showing `default` while none is set.
    fn color(&mut self, name: &str, which: Color, configured: Option<&str>, default: Rgb) {
        let tokens = self.tokens;
        let shown = configured.and_then(edit::rgb).unwrap_or(default);
        self.row(
            name,
            None,
            |ui, edits| {
                let mut rgb = shown;
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    edits.push(Edit::Color(which, Some(rgb)));
                }
                ui.label(
                    RichText::new(edit::hex(shown))
                        .monospace()
                        .color(tokens.muted),
                );
            },
            configured.map(|_| {
                (
                    format!("Reset to {}", edit::hex(default)),
                    vec![Edit::Color(which, None)],
                )
            }),
        );
    }
}

fn font(content: &mut Content<'_, '_>, view: &View<'_>) {
    let settings = view.settings;
    content.group(None, None, |rows| {
        let current = settings.font.family.as_deref();
        rows.row(
            "Family",
            Some("Installed fixed-width faces"),
            |ui, edits| {
                ComboBox::from_id_salt("family")
                    .selected_text(current.unwrap_or("System default"))
                    .width(240.0)
                    .height(320.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current.is_none(), "System default")
                            .clicked()
                        {
                            edits.push(Edit::FontFamily(None));
                        }
                        for family in view.families {
                            if ui
                                .selectable_label(current == Some(family.as_str()), family)
                                .clicked()
                            {
                                edits.push(Edit::FontFamily(Some(family.clone())));
                            }
                        }
                    });
            },
            current.map(|_| {
                (
                    "Reset to the system default".to_owned(),
                    vec![Edit::FontFamily(None)],
                )
            }),
        );
        let size = settings::font_size(settings);
        rows.row(
            "Size",
            None,
            |ui, edits| {
                let mut size = size;
                if slider(ui, &mut size, FONT_SIZES, 0.5, "px") {
                    edits.push(Edit::FontSize(Some(size)));
                }
            },
            settings.font.size_px.map(|_| {
                (
                    format!("Reset to {DEFAULT_FONT_SIZE_LOGICAL_PX} px"),
                    vec![Edit::FontSize(None)],
                )
            }),
        );
    });
}

/// A slider with its value after it, in a right-to-left row; whether the
/// user moved it.
fn slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    unit: &str,
) -> bool {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    ui.add_sized(
        vec2(56.0, 20.0),
        egui::Label::new(RichText::new(amount(*value, unit)).color(tokens.muted)),
    );
    ui.add(Slider::new(value, range).step_by(step).show_value(false))
        .changed()
}

/// `value` with its unit, without a decimal point it does not need.
fn amount(value: f32, unit: &str) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0} {unit}")
    } else {
        format!("{value:.1} {unit}")
    }
}

fn colors(content: &mut Content<'_, '_>, view: &View<'_>) {
    let settings = view.settings;
    let theme = &settings.theme;
    content.group(Some("Text and background"), None, |rows| {
        rows.color(
            "Foreground",
            Color::Foreground,
            theme.foreground.as_deref(),
            DEFAULT_FG_SRGB,
        );
        rows.color(
            "Background",
            Color::Background,
            theme.background.as_deref(),
            DEFAULT_BG_SRGB,
        );
        let opacity = settings.window.clamped_opacity();
        rows.row(
            "Opacity",
            Some("Turning a window translucent or opaque applies to new windows."),
            |ui, edits| {
                let mut percent = (opacity * 100.0).round();
                if slider(ui, &mut percent, 0.0..=100.0, 1.0, "%") {
                    edits.push(Edit::Opacity(Some(percent / 100.0)));
                }
            },
            (opacity < 1.0).then(|| ("Reset to 100 %".to_owned(), vec![Edit::Opacity(None)])),
        );
    });

    let palette = theme.palette.clone().into_overrides();
    let configured: Vec<usize> = (0..16)
        .filter(|index| u8::try_from(*index).is_ok_and(|slot| palette.contains_key(&slot)))
        .collect();
    content.group(
        Some("Palette"),
        Some("The sixteen colors programs pick from."),
        |rows| {
            for (name, range) in [("Normal", 0..8), ("Bright", 8..16)] {
                let last = range.end == 16;
                rows.row(
                    name,
                    None,
                    |ui, edits| {
                        for index in range.rev() {
                            let mut rgb = u8::try_from(index)
                                .ok()
                                .and_then(|slot| palette.get(&slot))
                                .and_then(|hex| edit::rgb(hex))
                                .unwrap_or(XTERM_PALETTE[index]);
                            let response = ui
                                .color_edit_button_srgb(&mut rgb)
                                .on_hover_text(PALETTE_NAMES[index].replace('_', " "));
                            if response.changed() {
                                edits.push(Edit::Color(Color::Palette(index), Some(rgb)));
                            }
                        }
                    },
                    (last && !configured.is_empty()).then(|| {
                        (
                            "Reset the palette".to_owned(),
                            configured
                                .iter()
                                .map(|index| Edit::Color(Color::Palette(*index), None))
                                .collect(),
                        )
                    }),
                );
            }
        },
    );
}

fn cursor(content: &mut Content<'_, '_>, view: &View<'_>) {
    let settings = view.settings;
    content.group(
        None,
        Some("The cursor's shape is the program's to choose."),
        |rows| {
            rows.color(
                "Color",
                Color::Cursor,
                settings.cursor.color.as_deref(),
                DEFAULT_FG_SRGB,
            );
            let current = settings.cursor.blink;
            rows.row(
                "Blink",
                None,
                |ui, edits| {
                    ComboBox::from_id_salt("blink")
                        .selected_text(blink(current))
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for mode in [
                                CursorBlinkMode::Program,
                                CursorBlinkMode::Always,
                                CursorBlinkMode::Never,
                            ] {
                                if ui.selectable_label(current == mode, blink(mode)).clicked()
                                    && current != mode
                                {
                                    edits.push(Edit::Blink(mode));
                                }
                            }
                        });
                },
                (current != CursorBlinkMode::Program).then(|| {
                    (
                        format!(
                            "Reset to {}",
                            blink(CursorBlinkMode::Program).to_lowercase()
                        ),
                        vec![Edit::Blink(CursorBlinkMode::Program)],
                    )
                }),
            );
        },
    );
}

fn blink(mode: CursorBlinkMode) -> &'static str {
    match mode {
        CursorBlinkMode::Program => "As the program asks",
        CursorBlinkMode::Always => "Always",
        CursorBlinkMode::Never => "Never",
    }
}

fn shell(content: &mut Content<'_, '_>, line: &mut String) {
    let tokens = content.tokens;
    content.group(None, None, |rows| {
        let refused = edit::shell(line).err();
        rows.row(
            "Command",
            Some("Runs in new sessions. Left empty, your login shell runs."),
            |ui, edits| {
                let response = ui.add(
                    TextEdit::singleline(line)
                        .hint_text("Login shell")
                        .desired_width(240.0)
                        .margin(vec2(8.0, 5.0)),
                );
                if response.lost_focus()
                    && let Ok(edit) = edit::shell(line)
                {
                    edits.push(edit);
                }
            },
            None,
        );
        if let Some(refused) = refused {
            rows.ui.label(
                RichText::new(refused.to_string())
                    .small()
                    .color(tokens.error),
            );
            rows.ui.add_space(8.0);
        }
    });
}

fn footer(content: &mut Content<'_, '_>, view: &View<'_>) {
    let tokens = content.tokens;
    let ui = &mut *content.ui;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("Saved in {}", view.path))
                .small()
                .color(tokens.muted),
        );
    });
    if let Some(problem) = view.problem {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new(problem).small().color(tokens.error));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_amount_is_shown_without_a_decimal_point() {
        assert_eq!(amount(14.0, "px"), "14 px");
        assert_eq!(amount(100.0, "%"), "100 %");
    }

    #[test]
    fn half_a_pixel_is_shown() {
        assert_eq!(amount(14.5, "px"), "14.5 px");
    }
}
