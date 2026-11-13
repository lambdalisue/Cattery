//! The Sessions window: the sessions Cattery's daemon keeps, to open in a
//! window again or to end.

use egui::{
    Align, Align2, CentralPanel, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Sense,
    Stroke, Ui, vec2,
};
use tokio::runtime::Runtime;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::look::{self, Tokens};
use crate::roster::{self, Row, State};
use crate::surface::Surface;

/// What a window event asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    /// Show the session in a window.
    Open(u128),
    /// End the session and its program.
    End(u128),
    Closed,
}

pub struct Sessions {
    surface: Surface,
    rows: Vec<Row>,
}

impl Sessions {
    pub fn open(event_loop: &ActiveEventLoop, runtime: &Runtime) -> anyhow::Result<Self> {
        let surface = Surface::open(
            event_loop,
            runtime,
            Window::default_attributes()
                .with_title("Cattery Sessions")
                .with_inner_size(LogicalSize::new(620.0, 460.0))
                .with_min_inner_size(LogicalSize::new(420.0, 260.0)),
        )?;
        Ok(Self {
            surface,
            rows: Vec::new(),
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
        if matches!(event, WindowEvent::CloseRequested) {
            return Asked::Closed;
        }
        if !self.surface.take(event) {
            return Asked::Nothing;
        }
        let rows = &self.rows;
        let mut asked = Asked::Nothing;
        self.surface.frame(|ui| asked = list(ui, rows));
        asked
    }
}

fn list(ui: &mut Ui, rows: &[Row]) -> Asked {
    let tokens = Tokens::of(ui.visuals().dark_mode);
    let mut asked = Asked::Nothing;
    CentralPanel::default()
        .frame(Frame::new().fill(tokens.window))
        .show(ui, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
            ui.painter().text(
                rect.left_center() + vec2(20.0, 0.0),
                Align2::LEFT_CENTER,
                "Sessions",
                look::bold(15.0),
                tokens.text,
            );
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                Stroke::new(1.0, tokens.separator),
            );
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
                        if rows.is_empty() {
                            ui.label(RichText::new("No sessions.").color(tokens.muted));
                        } else {
                            asked = group(ui, rows, tokens);
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "A window closed leaves its session here, running. \
                                 End stops the program in it.",
                            )
                            .small()
                            .color(tokens.muted),
                        );
                    });
            });
        });
    asked
}

fn group(ui: &mut Ui, rows: &[Row], tokens: Tokens) -> Asked {
    let mut asked = Asked::Nothing;
    Frame::new()
        .fill(tokens.group)
        .stroke(Stroke::new(1.0, tokens.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (at, row) in rows.iter().enumerate() {
                if at > 0 {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().hline(
                        rect.x_range(),
                        rect.center().y,
                        Stroke::new(1.0, tokens.separator),
                    );
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let room = (ui.available_width() - 150.0).max(160.0);
                    ui.vertical(|ui| {
                        ui.set_width(room);
                        ui.add(egui::Label::new(&row.title).truncate());
                        let detail = if row.place.is_empty() {
                            roster::describe(row.state)
                        } else {
                            format!("{} · {}", roster::describe(row.state), row.place)
                        };
                        ui.add(
                            egui::Label::new(RichText::new(detail).small().color(tokens.muted))
                                .truncate(),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("End").clicked() {
                            asked = Asked::End(row.id);
                        }
                        let open = match row.state {
                            State::Shown(_) => "Show",
                            _ => "Open",
                        };
                        if ui.button(open).clicked() {
                            asked = Asked::Open(row.id);
                        }
                    });
                });
                ui.add_space(8.0);
            }
        });
    asked
}
