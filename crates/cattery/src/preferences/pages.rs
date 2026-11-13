//! The Preferences window's pages: a sidebar to pick one, and the page as
//! groups of rows, each a setting's name on the left and its control on the
//! right, as macOS's own settings are laid out.

use std::path::Path;

use egui::{
    Align, Align2, CentralPanel, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, Panel,
    Rect, RichText, ScrollArea, Sense, Slider, Stroke, TextEdit, Ui, vec2,
};
use felis_client_core::config::CursorBlinkMode;
use felis_render_core::palette::{DEFAULT_FG_SRGB, PALETTE_NAMES};
use felis_render_wgpu::DEFAULT_FONT_SIZE_LOGICAL_PX;
use winit::window::Theme;

use super::look::{self, Tokens};
use super::{Change, Shown};
use crate::about::{self, Target};
use crate::appearance::{Appearance, Mode};
use crate::edit::{self, Color, Edit, Rgb};
use crate::settings;
use crate::themes::{Colors, Kind};

/// The font sizes, in logical pixels, felis accepts.
const FONT_SIZES: std::ops::RangeInclusive<f32> = 4.0..=72.0;

/// The width every control in a row is laid out to, so they line up.
const CONTROL_WIDTH: f32 = 220.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Theme,
    Font,
    Cursor,
    Shell,
    About,
}

impl Page {
    /// The pages about settings; About sits apart below them.
    const SETTINGS: [Self; 4] = [Self::Theme, Self::Font, Self::Cursor, Self::Shell];

    const fn title(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::Font => "Font",
            Self::Cursor => "Cursor",
            Self::Shell => "Shell",
            Self::About => "About",
        }
    }
}

/// A theme to choose, with its colours to preview it by.
pub struct ThemeChoice {
    pub name: String,
    pub kind: Kind,
    /// `None` for a theme that does not read.
    pub colors: Option<Colors>,
}

/// The files the settings are kept in.
pub struct Files<'a> {
    pub settings: &'a Path,
    pub appearance: &'a Path,
    pub themes: &'a Path,
}

/// What the pages show and where they keep what is being typed.
pub struct View<'a> {
    pub shown: &'a Shown<'a>,
    /// Installed families felis can draw a grid with.
    pub families: &'a [String],
    pub themes: &'a [ThemeChoice],
    pub themes_dir: &'a str,
    pub files: &'a Files<'a>,
    /// The icon, for the About page.
    pub icon: Option<&'a egui::TextureHandle>,
    /// The shell line as it is being typed.
    pub shell: &'a mut String,
    /// Why the last change was not saved.
    pub problem: Option<&'a str>,
    pub path: &'a str,
}

/// Draw the window on `page`; what the user changed in this frame.
pub fn show(ui: &mut Ui, page: &mut Page, mut view: View<'_>) -> Vec<Change> {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    Panel::left("pages")
        .exact_size(190.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(tokens.sidebar)
                .inner_margin(Margin::symmetric(10, 12)),
        )
        .show(ui, |ui| sidebar(ui, page, tokens));
    let mut changes = Vec::new();
    CentralPanel::default()
        .frame(Frame::new().fill(tokens.window))
        .show(ui, |ui| {
            title(ui, page.title(), tokens);
            ScrollArea::vertical().show(ui, |ui| {
                Frame::new()
                    .inner_margin(Margin {
                        left: 20,
                        right: 20,
                        top: 16,
                        bottom: 24,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let content = &mut Content {
                            ui,
                            tokens,
                            changes: &mut changes,
                        };
                        match page {
                            Page::Theme => theme(content, &view),
                            Page::Font => font(content, &view),
                            Page::Cursor => cursor(content, &view),
                            Page::Shell => shell(content, view.shell),
                            Page::About => about(content, &view),
                        }
                        footer(content, &mut view);
                    });
            });
        });
    changes
}

fn sidebar(ui: &mut Ui, page: &mut Page, tokens: Tokens) {
    ui.add_space(2.0);
    for item in Page::SETTINGS {
        sidebar_item(ui, page, item, tokens);
    }
    // As in Kanaemi and Arto, the page about the app sits apart at the bottom.
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        sidebar_item(ui, page, Page::About, tokens);
    });
}

fn sidebar_item(ui: &mut Ui, page: &mut Page, item: Page, tokens: Tokens) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let selected = *page == item;
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), tokens.accent);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), tokens.separator);
    }
    ui.painter().text(
        rect.left_center() + vec2(10.0, 0.0),
        Align2::LEFT_CENTER,
        item.title(),
        egui::FontId::proportional(13.0),
        if selected {
            tokens.on_accent
        } else {
            tokens.text
        },
    );
    if response.clicked() {
        *page = item;
    }
    ui.add_space(2.0);
}

fn title(ui: &mut Ui, title: &str, tokens: Tokens) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(20.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        look::bold(15.0),
        tokens.text,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, tokens.separator),
    );
}

/// A page being drawn, and what the user changed on it.
struct Content<'u, 'c> {
    ui: &'u mut Ui,
    tokens: Tokens,
    changes: &'c mut Vec<Change>,
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
                ui.add_space(4.0);
                ui.label(RichText::new(heading).font(look::bold(13.0)));
            });
            self.ui.add_space(2.0);
        }
        let changes = &mut *self.changes;
        Frame::new()
            .fill(tokens.group)
            .stroke(Stroke::new(1.0, tokens.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 0))
            .show(self.ui, |ui| {
                ui.set_width(ui.available_width());
                rows(&mut Rows {
                    ui,
                    tokens,
                    changes,
                    first: true,
                });
            });
        if let Some(note) = note {
            self.ui.add_space(4.0);
            self.ui.horizontal_wrapped(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(note).small().color(tokens.muted));
            });
        }
        self.ui.add_space(18.0);
    }
}

/// The rows of one group.
struct Rows<'u> {
    ui: &'u mut Ui,
    tokens: Tokens,
    changes: &'u mut Vec<Change>,
    first: bool,
}

impl Rows<'_> {
    /// A row naming a setting, with what it is under the name, its control
    /// on the right, and — while it is off its default — a way back to it.
    fn row(
        &mut self,
        name: &str,
        description: Option<&str>,
        control: impl FnOnce(&mut Ui, &mut Vec<Change>),
        reset: Option<(String, Vec<Change>)>,
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
        self.ui.add_space(8.0);
        let changes = &mut *self.changes;
        self.ui.horizontal(|ui| {
            ui.set_min_height(24.0);
            // The name and what it is take what the control leaves,
            // wrapping there rather than running under the control.
            let room = (ui.available_width() - CONTROL_WIDTH - 16.0).max(120.0);
            ui.vertical(|ui| {
                ui.set_width(room);
                ui.add_space(2.0);
                ui.label(name);
                if let Some(description) = description {
                    ui.add(
                        egui::Label::new(RichText::new(description).small().color(tokens.muted))
                            .wrap(),
                    );
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                control(ui, changes)
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
                        changes.extend(reset);
                    }
                });
        }
        self.ui.add_space(8.0);
    }

    /// A row picking `which` colour, showing `default` while none is set.
    fn color(&mut self, name: &str, which: Color, configured: Option<&str>, default: Rgb) {
        let tokens = self.tokens;
        let shown = configured.and_then(edit::rgb).unwrap_or(default);
        self.row(
            name,
            None,
            |ui, changes| {
                let mut rgb = shown;
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    changes.push(Change::Setting(Edit::Color(which, Some(rgb))));
                }
                ui.label(
                    RichText::new(edit::hex(shown))
                        .monospace()
                        .color(tokens.muted),
                );
            },
            configured.map(|_| {
                (
                    "Use the theme's color".to_owned(),
                    vec![Change::Setting(Edit::Color(which, None))],
                )
            }),
        );
    }
}

/// A pop-up button choosing among `choices`, as wide as every control.
fn choose<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    current: T,
    choices: &[(T, &str)],
) -> Option<T> {
    let label = choices
        .iter()
        .find(|(value, _)| *value == current)
        .map_or("", |(_, label)| label);
    let mut chosen = None;
    ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(CONTROL_WIDTH)
        .icon(chevrons)
        .show_ui(ui, |ui| {
            for (value, label) in choices {
                if ui.selectable_label(*value == current, *label).clicked() && *value != current {
                    chosen = Some(*value);
                }
            }
        });
    chosen
}

/// The up-and-down chevrons a macOS pop-up button carries.
fn chevrons(ui: &Ui, rect: Rect, visuals: &egui::style::WidgetVisuals, _open: bool) {
    let center = rect.center();
    let stroke = Stroke::new(1.2, visuals.fg_stroke.color);
    for direction in [-1.0, 1.0] {
        let tip = center + vec2(0.0, direction * 4.5);
        let base = center + vec2(0.0, direction * 1.5);
        ui.painter()
            .line_segment([base + vec2(-3.0, 0.0), tip], stroke);
        ui.painter()
            .line_segment([base + vec2(3.0, 0.0), tip], stroke);
    }
}

fn theme(content: &mut Content<'_, '_>, view: &View<'_>) {
    let shown = view.shown;
    let appearance = shown.appearance;
    let system = match shown.system {
        Some(Theme::Light) => "light",
        Some(Theme::Dark) => "dark",
        None => "unknown, so dark",
    };
    let describe = format!("System follows the system's appearance, now {system}.");
    content.group(None, None, |rows| {
        rows.row(
            "Appearance",
            Some(&describe),
            |ui, changes| {
                let choices = [
                    (Mode::System, "System"),
                    (Mode::Light, "Light"),
                    (Mode::Dark, "Dark"),
                ];
                if let Some(mode) = choose(ui, "appearance", appearance.mode, &choices) {
                    changes.push(Change::Appearance(Appearance {
                        mode,
                        ..appearance.clone()
                    }));
                }
            },
            None,
        );
        for kind in [Kind::Light, Kind::Dark] {
            let (name, describe, current) = match kind {
                Kind::Light => (
                    "Light theme",
                    "The colors while the appearance is light.",
                    &appearance.light_theme,
                ),
                Kind::Dark => (
                    "Dark theme",
                    "The colors while the appearance is dark.",
                    &appearance.dark_theme,
                ),
            };
            rows.row(
                name,
                Some(describe),
                |ui, changes| {
                    if let Some(theme) = choose_theme(ui, kind, current, view.themes) {
                        let mut chosen = appearance.clone();
                        match kind {
                            Kind::Light => chosen.light_theme = theme,
                            Kind::Dark => chosen.dark_theme = theme,
                        }
                        changes.push(Change::Appearance(chosen));
                    }
                },
                None,
            );
        }
        let describe = format!(
            "A theme of your own is a file in Ghostty's theme format, named as the theme, in {}.",
            view.themes_dir
        );
        rows.row(
            "Your themes",
            Some(&describe),
            |ui, changes| {
                if ui.button("Open Folder").clicked() {
                    changes.push(Change::Open(Target::Folder(
                        view.files.themes.to_path_buf(),
                    )));
                }
            },
            None,
        );
    });

    let opacity = shown.settings.window.clamped_opacity();
    content.group(Some("Window"), None, |rows| {
        rows.row(
            "Opacity",
            Some("Turning a window translucent or opaque applies to new windows."),
            |ui, changes| {
                let mut percent = (opacity * 100.0).round();
                if slider(ui, &mut percent, 0.0..=100.0, 1.0, "%") {
                    changes.push(Change::Setting(Edit::Opacity(Some(percent / 100.0))));
                }
            },
            (opacity < 1.0).then(|| {
                (
                    "Reset to 100 %".to_owned(),
                    vec![Change::Setting(Edit::Opacity(None))],
                )
            }),
        );
    });

    let overridden = overridden(shown);
    if !overridden.is_empty() {
        content.group(
            Some("Colors over the theme"),
            Some("Colors set in the settings file are drawn in place of the theme's."),
            |rows| {
                let names: Vec<String> = overridden.iter().map(|(name, _)| name.clone()).collect();
                let resets = overridden
                    .iter()
                    .map(|(_, color)| Change::Setting(Edit::Color(*color, None)))
                    .collect();
                rows.row(
                    &names.join(", "),
                    None,
                    |_, _| {},
                    Some(("Use the theme's colors".to_owned(), resets)),
                );
            },
        );
    }
}

/// The colours the settings file sets over the theme, by name.
fn overridden(shown: &Shown<'_>) -> Vec<(String, Color)> {
    let settings = shown.settings;
    let mut set = Vec::new();
    if settings.theme.foreground.is_some() {
        set.push(("Foreground".to_owned(), Color::Foreground));
    }
    if settings.theme.background.is_some() {
        set.push(("Background".to_owned(), Color::Background));
    }
    let palette = settings.theme.palette.clone().into_overrides();
    for (index, name) in PALETTE_NAMES.iter().enumerate() {
        if u8::try_from(index).is_ok_and(|slot| palette.contains_key(&slot)) {
            let name = name.replace('_', " ");
            set.push((name, Color::Palette(index)));
        }
    }
    set
}

/// A pop-up button choosing among the themes of `kind`, each shown with its
/// colours; the theme chosen, if another was.
fn choose_theme(ui: &mut Ui, kind: Kind, current: &str, themes: &[ThemeChoice]) -> Option<String> {
    let mut chosen = None;
    let colors = themes
        .iter()
        .find(|theme| theme.name == current)
        .and_then(|theme| theme.colors.as_ref());
    ui.horizontal(|ui| {
        ComboBox::from_id_salt(("theme", kind == Kind::Light))
            .selected_text(current)
            .width(CONTROL_WIDTH - 52.0)
            .height(360.0)
            .icon(chevrons)
            .show_ui(ui, |ui| {
                // A theme named but not found stays in view, as chosen.
                if !themes.iter().any(|theme| theme.name == current) {
                    ui.label(RichText::new(format!("{current} (not found)")).italics());
                }
                for theme in themes.iter().filter(|theme| theme.kind == kind) {
                    ui.horizontal(|ui| {
                        swatch(ui, theme.colors.as_ref());
                        if ui
                            .selectable_label(theme.name == current, &theme.name)
                            .clicked()
                            && theme.name != current
                        {
                            chosen = Some(theme.name.clone());
                        }
                    });
                }
            });
        swatch(ui, colors);
    });
    chosen
}

/// A small picture of a theme: its background, with its text colour and
/// four of its colours on it.
fn swatch(ui: &mut Ui, colors: Option<&Colors>) {
    let (rect, _) = ui.allocate_exact_size(vec2(50.0, 18.0), Sense::hover());
    let Some(colors) = colors else {
        return;
    };
    let rgb = |rgb: Option<Rgb>, fallback: Rgb| {
        let [r, g, b] = rgb.unwrap_or(fallback);
        Color32::from_rgb(r, g, b)
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(4),
        rgb(colors.background, [0x0d, 0x0d, 0x12]),
        Stroke::new(1.0, Color32::from_gray(128).gamma_multiply(0.4)),
        egui::StrokeKind::Inside,
    );
    for (at, index) in [1, 2, 4, 5].into_iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "four dots")]
        let x = rect.left() + 8.0 + at as f32 * 9.0;
        painter.circle_filled(
            egui::pos2(x, rect.center().y),
            3.0,
            rgb(colors.palette[index], [0x80, 0x80, 0x80]),
        );
    }
    painter.circle_filled(
        egui::pos2(rect.right() - 7.0, rect.center().y),
        2.0,
        rgb(colors.foreground, DEFAULT_FG_SRGB),
    );
}

fn font(content: &mut Content<'_, '_>, view: &View<'_>) {
    let settings = view.shown.settings;
    content.group(None, None, |rows| {
        let current = settings.font.family.as_deref();
        rows.row(
            "Family",
            Some("Installed fixed-width faces."),
            |ui, changes| {
                ComboBox::from_id_salt("family")
                    .selected_text(current.unwrap_or("System default"))
                    .width(CONTROL_WIDTH)
                    .height(360.0)
                    .icon(chevrons)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current.is_none(), "System default")
                            .clicked()
                        {
                            changes.push(Change::Setting(Edit::FontFamily(None)));
                        }
                        for family in view.families {
                            if ui
                                .selectable_label(current == Some(family.as_str()), family)
                                .clicked()
                            {
                                changes
                                    .push(Change::Setting(Edit::FontFamily(Some(family.clone()))));
                            }
                        }
                    });
            },
            current.map(|_| {
                (
                    "Reset to the system default".to_owned(),
                    vec![Change::Setting(Edit::FontFamily(None))],
                )
            }),
        );
        let size = settings::font_size(settings);
        rows.row(
            "Size",
            None,
            |ui, changes| {
                let mut size = size;
                if slider(ui, &mut size, FONT_SIZES, 0.5, "px") {
                    changes.push(Change::Setting(Edit::FontSize(Some(size))));
                }
            },
            settings.font.size_px.map(|_| {
                (
                    format!("Reset to {DEFAULT_FONT_SIZE_LOGICAL_PX} px"),
                    vec![Change::Setting(Edit::FontSize(None))],
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
        vec2(48.0, 18.0),
        egui::Label::new(RichText::new(amount(*value, unit)).color(tokens.muted)),
    );
    ui.spacing_mut().slider_width = CONTROL_WIDTH - 56.0;
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

fn cursor(content: &mut Content<'_, '_>, view: &View<'_>) {
    let settings = view.shown.settings;
    let theme_cursor = view
        .shown
        .drawn
        .cursor
        .color
        .as_deref()
        .and_then(edit::rgb)
        .unwrap_or(DEFAULT_FG_SRGB);
    content.group(
        None,
        Some("The cursor's shape is the program's to choose."),
        |rows| {
            rows.color(
                "Color",
                Color::Cursor,
                settings.cursor.color.as_deref(),
                theme_cursor,
            );
            let current = settings.cursor.blink;
            rows.row(
                "Blink",
                None,
                |ui, changes| {
                    let choices = [
                        (CursorBlinkMode::Program, "As the program asks"),
                        (CursorBlinkMode::Always, "Always"),
                        (CursorBlinkMode::Never, "Never"),
                    ];
                    if let Some(mode) = choose(ui, "blink", current, &choices) {
                        changes.push(Change::Setting(Edit::Blink(mode)));
                    }
                },
                (current != CursorBlinkMode::Program).then(|| {
                    (
                        "Reset to as the program asks".to_owned(),
                        vec![Change::Setting(Edit::Blink(CursorBlinkMode::Program))],
                    )
                }),
            );
        },
    );
}

fn about(content: &mut Content<'_, '_>, view: &View<'_>) {
    let tokens = content.tokens;
    content.ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        if let Some(icon) = view.icon {
            ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(96.0, 96.0)));
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Cattery").font(look::bold(20.0)));
        ui.label(RichText::new(format!("Version {}", about::VERSION)).color(tokens.muted));
        if let Some(felis) = about::felis() {
            let short = felis.get(..7).unwrap_or(felis);
            ui.label(RichText::new(format!("felis {short}")).color(tokens.muted));
        }
        ui.add_space(6.0);
        ui.label("A GUI terminal app built on the felis terminal daemon.");
        ui.add_space(16.0);
    });
    let files = view.files;
    content.group(Some("Files"), None, |rows| {
        for (name, file) in [
            ("Settings", files.settings),
            ("Appearance", files.appearance),
        ] {
            rows.row(
                name,
                Some(&file.display().to_string()),
                |ui, changes| {
                    if ui.button(reveal_label()).clicked() {
                        changes.push(Change::Open(Target::Reveal(file.to_path_buf())));
                    }
                    if ui.button("Open").clicked() {
                        changes.push(Change::Open(Target::Edit(file.to_path_buf())));
                    }
                },
                None,
            );
        }
        rows.row(
            "Themes",
            Some(&files.themes.display().to_string()),
            |ui, changes| {
                if ui.button("Open Folder").clicked() {
                    changes.push(Change::Open(Target::Folder(files.themes.to_path_buf())));
                }
            },
            None,
        );
    });
    content.group(Some("Links"), None, |rows| {
        for (name, describe, url) in [
            (
                "Source code",
                "Cattery on GitHub.",
                about::REPOSITORY.to_owned(),
            ),
            (
                "Report a problem",
                "Cattery's issues on GitHub.",
                format!("{}/issues", about::REPOSITORY),
            ),
            (
                "felis",
                "The terminal daemon Cattery's sessions live in.",
                "https://github.com/felis-terminal/felis".to_owned(),
            ),
        ] {
            rows.row(
                name,
                Some(describe),
                |ui, changes| {
                    if ui.button("Open").clicked() {
                        changes.push(Change::Open(Target::Url(url)));
                    }
                },
                None,
            );
        }
    });
}

/// What the platform's file manager calls showing a file in it.
const fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else {
        "Show Folder"
    }
}

fn shell(content: &mut Content<'_, '_>, line: &mut String) {
    let tokens = content.tokens;
    content.group(None, None, |rows| {
        let refused = edit::shell(line).err();
        rows.row(
            "Command",
            Some("Runs in new windows. Left empty, your login shell runs."),
            |ui, changes| {
                let response = ui.add(
                    TextEdit::singleline(line)
                        .hint_text("Login shell")
                        .desired_width(CONTROL_WIDTH)
                        .margin(vec2(6.0, 3.0)),
                );
                if response.lost_focus()
                    && let Ok(edit) = edit::shell(line)
                {
                    changes.push(Change::Setting(edit));
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

fn footer(content: &mut Content<'_, '_>, view: &mut View<'_>) {
    let tokens = content.tokens;
    let ui = &mut *content.ui;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("Settings are kept in {}", view.path))
                .small()
                .color(tokens.muted),
        );
    });
    if let Some(problem) = view.problem {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(4.0);
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
