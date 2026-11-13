//! The Preferences window: Cattery's settings as controls, each change
//! written to the file it belongs in as it is made.

mod look;
mod pages;

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::ViewportId;
use egui_wgpu::winit::Painter;
use egui_wgpu::{RendererOptions, WgpuConfiguration, WgpuSetup};
use felis_client_core::config::EffectiveConfig;
use tokio::runtime::Runtime;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Theme, Window, WindowId};

use self::pages::{Page, ThemeChoice, View};
use crate::appearance::{self, Appearance};
use crate::edit::{self, Edit};
use crate::store::{self, Store};
use crate::themes;

/// What the Preferences window shows.
pub struct Shown<'a> {
    /// The felis settings as their file says them.
    pub settings: &'a EffectiveConfig,
    pub appearance: &'a Appearance,
    /// The settings with the theme in force under them.
    pub drawn: &'a EffectiveConfig,
    /// The system's appearance, where the platform tells it.
    pub system: Option<Theme>,
}

/// One change made in the window.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Setting(Edit),
    Appearance(Appearance),
    /// Show the folder of the user's own themes.
    OpenThemes,
}

/// What a window event did to the Preferences window.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// A settings file was written.
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
    appearance_path: PathBuf,
    themes_dir: PathBuf,
    page: Page,
    /// Installed families felis can draw a grid with, by name.
    families: Vec<String>,
    /// The themes to choose from.
    themes: Vec<ThemeChoice>,
    /// The shell line as it is being typed.
    shell: String,
    /// Why the last change was not saved.
    problem: Option<String>,
}

impl Preferences {
    /// Open the window on the felis settings file at `path`, beside which
    /// the appearance file and the themes folder are.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        path: PathBuf,
        shown: &Shown<'_>,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Cattery Preferences")
                    .with_inner_size(LogicalSize::new(760.0, 560.0))
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
        let themes_dir = appearance::themes_dir(&path);
        Ok(Self {
            window,
            context,
            input,
            painter,
            appearance_path: appearance::path(&path),
            themes: choices(&themes_dir),
            themes_dir,
            store: Store::new(path),
            page: Page::default(),
            families: monospace_families(),
            shell: edit::shell_line(&shown.settings.shell.command),
            problem: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn focus(&self) {
        self.window.focus_window();
    }

    /// Show the settings as their files now say them.
    pub fn reread(&mut self, shown: &Shown<'_>) {
        if !self.store.is_as_written() {
            self.shell = edit::shell_line(&shown.settings.shell.command);
        }
        self.themes = choices(&self.themes_dir);
        self.window.request_redraw();
    }

    pub fn window_event(&mut self, event: &WindowEvent, shown: &Shown<'_>) -> Outcome {
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
            WindowEvent::RedrawRequested => return self.draw(shown),
            _ => {}
        }
        if self.input.on_window_event(&self.window, event).repaint {
            self.window.request_redraw();
        }
        Outcome::Nothing
    }

    fn draw(&mut self, shown: &Shown<'_>) -> Outcome {
        let raw = self.input.take_egui_input(&self.window);
        let path = self
            .store
            .path()
            .parent()
            .unwrap_or(self.store.path())
            .display()
            .to_string();
        let themes_dir = self.themes_dir.display().to_string();
        let mut changes = Vec::new();
        let mut output = self.context.run_ui(raw, |ui| {
            changes = pages::show(
                ui,
                &mut self.page,
                View {
                    shown,
                    families: &self.families,
                    themes: &self.themes,
                    themes_dir: &themes_dir,
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
        if changes.is_empty() {
            return Outcome::Nothing;
        }
        match self.save(&changes) {
            Ok(saved) => {
                self.problem = None;
                if saved {
                    Outcome::Saved
                } else {
                    Outcome::Nothing
                }
            }
            Err(err) => {
                self.problem = Some(format!("{err:#}"));
                self.window.request_redraw();
                Outcome::Nothing
            }
        }
    }

    /// Make `changes`; whether a settings file was written.
    fn save(&mut self, changes: &[Change]) -> anyhow::Result<bool> {
        let edits: Vec<&Edit> = changes
            .iter()
            .filter_map(|change| match change {
                Change::Setting(edit) => Some(edit),
                _ => None,
            })
            .collect();
        let mut saved = false;
        if !edits.is_empty() {
            let mut doc = self.store.read()?;
            for edit in edits {
                edit::apply(&mut doc, edit);
            }
            self.store.write(&doc)?;
            saved = true;
        }
        for change in changes {
            match change {
                Change::Setting(_) => {}
                Change::Appearance(appearance) => {
                    write_appearance(&self.appearance_path, appearance)?;
                    saved = true;
                }
                Change::OpenThemes => open_folder(&self.themes_dir)?,
            }
        }
        Ok(saved)
    }
}

/// The themes in `dir` and Cattery's, to choose from.
fn choices(dir: &Path) -> Vec<ThemeChoice> {
    themes::listed(dir)
        .into_iter()
        .map(|(name, kind)| ThemeChoice {
            colors: themes::colors(&name, dir).ok(),
            name,
            kind,
        })
        .collect()
}

fn write_appearance(path: &Path, appearance: &Appearance) -> anyhow::Result<()> {
    let before = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let text = appearance
        .written_into(&before)
        .map_err(|err| anyhow::anyhow!("{}: {err}", path.display()))?;
    store::replace(path, &text)
}

/// Show `dir` in the platform's file manager, making it first.
fn open_folder(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(dir).spawn()?;
    Ok(())
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
