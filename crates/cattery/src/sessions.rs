//! The session palette: the sessions Cattery's daemon keeps, narrowed by
//! what is typed, to open in a window again or to end, and a new window.

use egui::{
    Align2, CentralPanel, CornerRadius, Frame, Key, Margin, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Ui, vec2,
};
use tokio::runtime::Runtime;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::look::Tokens;
use crate::roster::{self, Row};
use crate::surface::Surface;

/// What a window event asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    /// A new window, on a new session.
    New,
    /// Show the session in a window.
    Open(u128),
    /// End the session and its program.
    End(u128),
    Closed,
}

/// The palette's size, in logical pixels.
const SIZE: LogicalSize<f64> = LogicalSize::new(560.0, 380.0);

/// The window a palette opens over: where it is, how big, and at what scale.
pub type Over = (PhysicalPosition<i32>, PhysicalSize<u32>, f64);

pub struct Sessions {
    surface: Surface,
    rows: Vec<Row>,
    query: String,
    /// The entry chosen: 0 is the new window, then the matching sessions.
    chosen: usize,
    /// Whether the palette has been in front.
    focused: bool,
}

impl Sessions {
    /// Open the palette over the window `over`, where there is one.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        over: Option<Over>,
    ) -> anyhow::Result<Self> {
        let mut attributes = Window::default_attributes()
            .with_title("Cattery Sessions")
            .with_inner_size(SIZE)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        if let Some(over) = over {
            attributes = attributes.with_position(placed(over));
        }
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS as _;
            // A title bar left in place but hidden keeps macOS's rounded
            // corners and shadow.
            attributes = attributes
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_titlebar_buttons_hidden(true)
                .with_fullsize_content_view(true);
        }
        #[cfg(not(target_os = "macos"))]
        {
            attributes = attributes.with_decorations(false);
        }
        let surface = Surface::open(event_loop, runtime, attributes)?;
        Ok(Self {
            surface,
            rows: Vec::new(),
            query: String::new(),
            chosen: 0,
            focused: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.surface.id()
    }

    pub fn focus(&self) {
        self.surface.focus();
    }

    /// List `rows` from now on.
    pub fn show(&mut self, rows: Vec<Row>) {
        if rows != self.rows {
            self.rows = rows;
            self.surface.redraw();
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent) -> Asked {
        match event {
            WindowEvent::CloseRequested => return Asked::Closed,
            WindowEvent::Focused(true) => self.focused = true,
            // A palette is gone once the user turns elsewhere, which is not
            // yet so before it was ever in front.
            WindowEvent::Focused(false) if self.focused => return Asked::Closed,
            _ => {}
        }
        if !self.surface.take(event) {
            return Asked::Nothing;
        }
        let (rows, query, chosen) = (&self.rows, &mut self.query, &mut self.chosen);
        let mut asked = Asked::Nothing;
        self.surface
            .frame(|ui| asked = palette(ui, rows, query, chosen));
        asked
    }
}

/// Where the palette goes over the window `over`: centred across it, a
/// fifth of the way down.
fn placed((position, size, scale): Over) -> PhysicalPosition<i32> {
    let width = SIZE.width * scale;
    let offset = |value: f64| -> i32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a window is a few thousand pixels"
        )]
        let offset = value.round() as i32;
        offset
    };
    PhysicalPosition::new(
        position.x + offset((f64::from(size.width) - width) / 2.0),
        position.y + offset(f64::from(size.height) / 5.0),
    )
}

fn palette(ui: &mut Ui, rows: &[Row], query: &mut String, chosen: &mut usize) -> Asked {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    let matching = roster::filtered(rows, query);
    let entries = matching.len() + 1;
    *chosen = (*chosen).min(entries - 1);
    let end_chord = if cfg!(target_os = "macos") {
        egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT
    };
    let mut asked = Asked::Nothing;
    ui.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, Key::ArrowDown) {
            *chosen = (*chosen + 1) % entries;
        }
        if input.consume_key(egui::Modifiers::NONE, Key::ArrowUp) {
            *chosen = (*chosen + entries - 1) % entries;
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
            asked = Asked::Closed;
        }
        if input.consume_key(end_chord, Key::Backspace)
            && let Some(row) = chosen.checked_sub(1).and_then(|at| matching.get(at))
        {
            asked = Asked::End(row.id);
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
            asked = entry(*chosen, &matching);
        }
    });
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(tokens.group)
                .inner_margin(Margin::symmetric(12, 12)),
        )
        .show(ui, |ui| {
            // The macOS title bar is hidden over the top of the palette.
            if cfg!(target_os = "macos") {
                ui.add_space(14.0);
            }
            let typed = ui.add(
                TextEdit::singleline(query)
                    .hint_text("Search sessions")
                    .font(egui::FontId::proportional(17.0))
                    .frame(Frame::NONE)
                    .desired_width(f32::INFINITY),
            );
            typed.request_focus();
            if typed.changed() {
                *chosen = 0;
            }
            ui.add_space(8.0);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke::new(1.0, tokens.separator),
            );
            ui.add_space(6.0);
            let footer = 22.0;
            ScrollArea::vertical()
                .max_height(ui.available_height() - footer)
                .show(ui, |ui| {
                    let new = item(
                        ui,
                        tokens,
                        *chosen == 0,
                        "New Window",
                        "A new session in your home directory",
                    );
                    if new.hovered() {
                        *chosen = 0;
                    }
                    if new.clicked() {
                        asked = Asked::New;
                    }
                    for (at, row) in matching.iter().enumerate() {
                        let detail = if row.place.is_empty() {
                            roster::describe(row.state)
                        } else {
                            format!("{} · {}", roster::describe(row.state), row.place)
                        };
                        let response = item(ui, tokens, *chosen == at + 1, &row.title, &detail);
                        if response.hovered() {
                            *chosen = at + 1;
                        }
                        if response.clicked() {
                            asked = Asked::Open(row.id);
                        }
                    }
                });
            ui.add_space(4.0);
            let (open, end) = if cfg!(target_os = "macos") {
                ("Return", "⌘⌫")
            } else {
                ("Enter", "Ctrl+Shift+Backspace")
            };
            ui.label(
                RichText::new(format!("↑↓ Choose   {open} Open   {end} End   Esc Close"))
                    .small()
                    .color(tokens.muted),
            );
        });
    asked
}

/// What choosing the entry `at` asks for.
fn entry(at: usize, matching: &[&Row]) -> Asked {
    match at.checked_sub(1).and_then(|at| matching.get(at)) {
        None => Asked::New,
        Some(row) => Asked::Open(row.id),
    }
}

/// One entry: a name, and a line under it; drawn as chosen when it is.
fn item(ui: &mut Ui, tokens: Tokens, chosen: bool, name: &str, detail: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click());
    let (text, muted) = if chosen {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), tokens.accent);
        (tokens.on_accent, tokens.on_accent.gamma_multiply(0.8))
    } else {
        (tokens.text, tokens.muted)
    };
    let left = rect.left() + 10.0;
    ui.painter().text(
        egui::pos2(left, rect.top() + 13.0),
        Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        text,
    );
    ui.painter().text(
        egui::pos2(left, rect.top() + 29.0),
        Align2::LEFT_CENTER,
        detail,
        egui::FontId::proportional(11.0),
        muted,
    );
    if chosen {
        ui.scroll_to_rect(rect, None);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster::State;

    fn row(id: u128) -> Row {
        Row {
            id,
            title: String::new(),
            place: String::new(),
            state: State::Shown(1),
        }
    }

    #[test]
    fn the_first_entry_is_a_new_window_and_the_rest_are_sessions() {
        let (a, b) = (row(7), row(9));
        let matching = [&a, &b];
        assert_eq!(entry(0, &matching), Asked::New);
        assert_eq!(entry(1, &matching), Asked::Open(7));
        assert_eq!(entry(2, &matching), Asked::Open(9));
    }

    #[test]
    fn the_palette_sits_centred_a_fifth_of_the_way_down_the_window() {
        let at = placed((
            PhysicalPosition::new(100, 50),
            PhysicalSize::new(1560, 1000),
            2.0,
        ));
        assert_eq!(at, PhysicalPosition::new(100 + 220, 50 + 200));
    }
}
