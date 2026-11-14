//! A window's menu where the platform draws none Cattery can show: a small
//! window of Cattery's own at the pointer, drawn with egui.
//!
//! On Linux, muda draws menus only into GTK windows, which winit's are
//! not, so the menu macOS and Windows show natively is drawn here instead.

use egui::{
    Align2, CentralPanel, CornerRadius, FontId, Frame, Key, Margin, Rect, Sense, Stroke, vec2,
};
use tokio::runtime::Runtime;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::keys::Command;
use crate::look::Tokens;
use crate::menu::Line;
use crate::surface::Surface;

/// What the menu asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    Run(Command),
    Closed,
}

const WIDTH: f64 = 260.0;
const ITEM: f64 = 24.0;
const SEPARATOR: f64 = 9.0;
const PADDING: f64 = 5.0;
/// The width of a session's End button.
const END: f32 = 44.0;

pub struct Popup {
    surface: Surface,
    lines: Vec<Line>,
    /// Whether the menu has been in front.
    focused: bool,
}

impl Popup {
    /// Open a menu of `lines` at `at` on the screen, where the platform
    /// lets a window be placed.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        lines: Vec<Line>,
        at: Option<PhysicalPosition<i32>>,
    ) -> anyhow::Result<Self> {
        let mut attributes = Window::default_attributes()
            .with_title("Cattery")
            .with_inner_size(size(&lines))
            .with_resizable(false)
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        if let Some(at) = at {
            attributes = attributes.with_position(at);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use winit::platform::x11::{WindowAttributesExtX11 as _, WindowType};
            // A window manager keeps a pop-up menu off the taskbar, and
            // draws it where it is put.
            attributes = attributes.with_x11_window_type(vec![WindowType::PopupMenu]);
        }
        Ok(Self {
            surface: Surface::open(event_loop, runtime, attributes)?,
            lines,
            focused: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.surface.id()
    }

    pub fn window_event(&mut self, event: &WindowEvent) -> Asked {
        match event {
            WindowEvent::CloseRequested => return Asked::Closed,
            WindowEvent::Focused(true) => self.focused = true,
            // A menu is gone once the user turns elsewhere, which is not yet
            // so before it was ever in front.
            WindowEvent::Focused(false) if self.focused => return Asked::Closed,
            _ => {}
        }
        if !self.surface.take(event) {
            return Asked::Nothing;
        }
        let lines = &self.lines;
        let mut asked = Asked::Nothing;
        self.surface.frame(|ui| asked = menu(ui, lines));
        asked
    }
}

/// The size a menu of `lines` takes, in logical pixels.
fn size(lines: &[Line]) -> LogicalSize<f64> {
    let height = lines
        .iter()
        .map(|line| match line {
            Line::Item(..) | Line::Session(_) => ITEM,
            Line::Separator => SEPARATOR,
        })
        .sum::<f64>();
    LogicalSize::new(WIDTH, height + 2.0 * PADDING)
}

fn menu(ui: &mut egui::Ui, lines: &[Line]) -> Asked {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
        return Asked::Closed;
    }
    let mut asked = Asked::Nothing;
    #[expect(clippy::cast_possible_truncation, reason = "a few pixels")]
    let padding = PADDING as i8;
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(tokens.group)
                .stroke(Stroke::new(1.0, tokens.border))
                .inner_margin(Margin::same(padding)),
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for line in lines {
                match line {
                    Line::Item(command, label) => {
                        if row(ui, tokens, label, None).0 {
                            asked = Asked::Run(*command);
                        }
                    }
                    Line::Session(listed) => {
                        let (chosen, ended) = row(ui, tokens, &listed.title, Some("End"));
                        if ended {
                            asked = Asked::Run(Command::EndSessionOf(listed.id));
                        } else if chosen {
                            asked = Asked::Run(Command::ShowSession(listed.id));
                        }
                    }
                    Line::Separator => {
                        #[expect(clippy::cast_possible_truncation, reason = "a row's height")]
                        let height = SEPARATOR as f32;
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), height),
                            Sense::hover(),
                        );
                        ui.painter().hline(
                            rect.x_range().shrink(6.0),
                            rect.center().y,
                            Stroke::new(1.0, tokens.separator),
                        );
                    }
                }
            }
        });
    asked
}

/// A row reading `label`, with a button reading `button` at its right where
/// there is one; whether the row, and whether the button, was clicked.
fn row(ui: &mut egui::Ui, tokens: Tokens, label: &str, button: Option<&str>) -> (bool, bool) {
    #[expect(clippy::cast_possible_truncation, reason = "a row's height")]
    let height = ITEM as f32;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), tokens.accent);
    }
    let color = if hovered {
        tokens.on_accent
    } else {
        tokens.text
    };
    let pressed = button.is_some_and(|button| {
        let at = Rect::from_min_max(rect.right_top() - vec2(END, 0.0), rect.right_bottom());
        let pressed = ui.interact(at, response.id.with("button"), Sense::click());
        if pressed.hovered() {
            ui.painter()
                .rect_filled(at.shrink(2.0), CornerRadius::same(4), tokens.error);
        }
        ui.painter().text(
            at.center(),
            Align2::CENTER_CENTER,
            button,
            FontId::proportional(11.0),
            if pressed.hovered() {
                tokens.on_accent
            } else {
                color
            },
        );
        pressed.clicked()
    });
    let room = rect.width() - 20.0 - if button.is_some() { END } else { 0.0 };
    let text = ui.painter().layout_job(crate::palette::one_line(
        egui::text::LayoutJob::simple_singleline(
            label.to_owned(),
            FontId::proportional(13.0),
            color,
        ),
        room,
    ));
    ui.painter().galley(
        rect.left_center() + vec2(10.0, -text.size().y / 2.0),
        text,
        color,
    );
    (response.clicked() && !pressed, pressed)
}
