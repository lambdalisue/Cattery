//! The Preferences window: Cattery's settings as controls, each change
//! written to the settings file as it is made.

mod look;
mod pages;

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use egui::ViewportId;
use egui_wgpu::winit::Painter;
use egui_wgpu::{RendererOptions, WgpuConfiguration, WgpuSetup};
use felis_client_core::config::EffectiveConfig;
use tokio::runtime::Runtime;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use self::pages::{Page, View};
use crate::edit::{self, Edit};
use crate::store::Store;

/// What a window event did to the Preferences window.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// The settings file was written.
    Saved,
    /// The window was asked to close.
    Closed,
}

pub struct Preferences {
    window: Arc<Window>,
    context: egui::Context,
    input: egui_winit::State,
    painter: Painter,
    store: Store,
    page: Page,
    /// Installed families felis can draw a grid with, by name.
    families: Vec<String>,
    /// The shell line as it is being typed.
    shell: String,
    /// Why the last change was not saved.
    problem: Option<String>,
}

impl Preferences {
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        path: PathBuf,
        settings: &EffectiveConfig,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Cattery Preferences")
                    .with_inner_size(LogicalSize::new(760.0, 540.0))
                    .with_min_inner_size(LogicalSize::new(600.0, 400.0)),
            )?,
        );
        let context = egui::Context::default();
        look::install(&context);
        let config = WgpuConfiguration {
            wgpu_setup: WgpuSetup::from_display_handle(event_loop.owned_display_handle()),
            ..WgpuConfiguration::default()
        };
        let mut painter = runtime.block_on(Painter::new(
            context.clone(),
            config,
            false,
            RendererOptions::default(),
        ));
        runtime.block_on(painter.set_window(ViewportId::ROOT, Some(Arc::clone(&window))))?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a display's scale factor is a small number"
        )]
        let scale = window.scale_factor() as f32;
        let input = egui_winit::State::new(
            context.clone(),
            ViewportId::ROOT,
            &window,
            Some(scale),
            window.theme(),
            painter.max_texture_side(),
        );
        Ok(Self {
            window,
            context,
            input,
            painter,
            store: Store::new(path),
            page: Page::default(),
            families: monospace_families(),
            shell: edit::shell_line(&settings.shell.command),
            problem: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn focus(&self) {
        self.window.focus_window();
    }

    /// Show `settings` as the file now says them, unless they are this
    /// window's own.
    pub fn reread(&mut self, settings: &EffectiveConfig) {
        if !self.store.is_as_written() {
            self.shell = edit::shell_line(&settings.shell.command);
        }
        self.window.request_redraw();
    }

    pub fn window_event(&mut self, event: &WindowEvent, settings: &EffectiveConfig) -> Outcome {
        match event {
            WindowEvent::CloseRequested => return Outcome::Closed,
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter
                        .on_window_resized(ViewportId::ROOT, width, height);
                }
            }
            WindowEvent::RedrawRequested => return self.draw(settings),
            _ => {}
        }
        if self.input.on_window_event(&self.window, event).repaint {
            self.window.request_redraw();
        }
        Outcome::Nothing
    }

    fn draw(&mut self, settings: &EffectiveConfig) -> Outcome {
        let raw = self.input.take_egui_input(&self.window);
        let path = self.store.path().display().to_string();
        let mut edits = Vec::new();
        let mut output = self.context.run_ui(raw, |ui| {
            edits = pages::show(
                ui,
                &mut self.page,
                View {
                    settings,
                    families: &self.families,
                    shell: &mut self.shell,
                    problem: self.problem.as_deref(),
                    path: &path,
                },
            );
        });
        self.input
            .handle_platform_output(&self.window, output.platform_output);
        let shapes = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            output.pixels_per_point,
            [0.0, 0.0, 0.0, 1.0],
            &shapes,
            &mut output.textures_delta,
            Vec::new(),
            &self.window,
        );
        if output
            .viewport_output
            .get(&ViewportId::ROOT)
            .is_some_and(|viewport| viewport.repaint_delay.is_zero())
        {
            self.window.request_redraw();
        }
        if edits.is_empty() {
            return Outcome::Nothing;
        }
        match self.save(&edits) {
            Ok(()) => {
                self.problem = None;
                Outcome::Saved
            }
            Err(err) => {
                self.problem = Some(format!("{err:#}"));
                self.window.request_redraw();
                Outcome::Nothing
            }
        }
    }

    fn save(&mut self, edits: &[Edit]) -> anyhow::Result<()> {
        let mut doc = self.store.read()?;
        for edit in edits {
            edit::apply(&mut doc, edit);
        }
        self.store.write(&doc)
    }
}

/// The families a grid can be drawn with, for the font list; empty when
/// the platform cannot list them, which leaves only the default.
fn monospace_families() -> Vec<String> {
    match felis_render_wgpu::installed_families() {
        Ok(families) => families
            .into_iter()
            .filter(|family| family.monospace)
            .map(|family| family.name)
            .collect(),
        Err(err) => {
            tracing::warn!(?err, "the installed fonts were not listed");
            Vec::new()
        }
    }
}
