//! A window's menu where the platform draws none Cattery can show: a small
//! window of Cattery's own at the pointer, drawn with egui.
//!
//! On Linux, muda draws menus only into GTK windows, which winit's are
//! not, so the menu macOS and Windows show natively is drawn here instead.

use egui::{CentralPanel, CornerRadius, Frame, Key, Margin, Sense, Stroke, vec2};
use tokio::runtime::Runtime;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::keys::Command;
use crate::look::Tokens;
use crate::menu::Entry;
use crate::surface::Surface;

/// What the menu asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    Run(Command),
    Closed,
}

const WIDTH: f64 = 220.0;
const ITEM: f64 = 24.0;
const SEPARATOR: f64 = 9.0;
const PADDING: f64 = 5.0;

pub struct Popup {
    surface: Surface,
    entries: &'static [Entry],
    /// Whether the menu has been in front.
    focused: bool,
}

impl Popup {
    /// Open a menu of `entries` at `at` on the screen, where the platform
    /// lets a window be placed.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        entries: &'static [Entry],
        at: Option<PhysicalPosition<i32>>,
    ) -> anyhow::Result<Self> {
        let mut attributes = Window::default_attributes()
            .with_title("Cattery")
            .with_inner_size(size(entries))
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
            entries,
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
        let entries = self.entries;
        let mut asked = Asked::Nothing;
        self.surface.frame(|ui| asked = menu(ui, entries));
        asked
    }
}

/// The size a menu of `entries` takes, in logical pixels.
fn size(entries: &[Entry]) -> LogicalSize<f64> {
    let height = entries
        .iter()
        .map(|entry| match entry {
            Entry::Item(..) => ITEM,
            Entry::Separator => SEPARATOR,
        })
        .sum::<f64>();
    LogicalSize::new(WIDTH, height + 2.0 * PADDING)
}

fn menu(ui: &mut egui::Ui, entries: &[Entry]) -> Asked {
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
            for entry in entries {
                match *entry {
                    Entry::Item(command, label) => {
                        #[expect(clippy::cast_possible_truncation, reason = "a row's height")]
                        let height = ITEM as f32;
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(ui.available_width(), height),
                            Sense::click(),
                        );
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
                        ui.painter().text(
                            rect.left_center() + vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::proportional(13.0),
                            color,
                        );
                        if response.clicked() {
                            asked = Asked::Run(command);
                        }
                    }
                    Entry::Separator => {
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
