//! The Preferences window: Cattery's settings as controls, each change
//! written to the file it belongs in as it is made.

mod pages;

use std::path::{Path, PathBuf};

use felis_client_core::config::EffectiveConfig;
use tokio::runtime::Runtime;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Theme, Window, WindowButtons, WindowId};

pub use self::pages::Page;
use self::pages::{Files, ThemeChoice, View};
use crate::about::{self, Target};
use crate::appearance::{self, Appearance};
use crate::edit::{self, Edit};
use crate::store::{self, Store};
use crate::surface::Surface;
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
    /// Show a file, a folder or a page in the app that shows it.
    Open(Target),
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
    surface: Surface,
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
    /// The icon, as the About page shows it.
    icon: Option<egui::TextureHandle>,
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
        let surface = Surface::open(
            event_loop,
            runtime,
            Window::default_attributes()
                .with_title("Cattery Preferences")
                .with_window_icon(crate::icon::window(shown.appearance.look(shown.system)))
                .with_inner_size(LogicalSize::new(760.0, 560.0))
                .with_min_inner_size(LogicalSize::new(600.0, 400.0))
                // A settings window is not one to fill the screen with, as
                // the platform's own settings windows are not.
                .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE),
        )?;
        let themes_dir = appearance::themes_dir(&path);
        let icon = icon_texture(surface.context(), shown.appearance.look(shown.system));
        Ok(Self {
            surface,
            appearance_path: appearance::path(&path),
            themes: choices(&themes_dir),
            themes_dir,
            store: Store::new(path),
            page: Page::default(),
            families: crate::fonts::with_installed(monospace_families()),
            shell: edit::shell_line(&shown.settings.shell.command),
            problem: None,
            icon,
        })
    }

    pub fn id(&self) -> WindowId {
        self.surface.id()
    }

    pub fn focus(&self) {
        self.surface.focus();
    }

    pub fn show_page(&mut self, page: Page) {
        self.page = page;
        self.surface.redraw();
    }

    /// Show the settings as their files now say them.
    pub fn reread(&mut self, shown: &Shown<'_>) {
        if !self.store.is_as_written() {
            self.shell = edit::shell_line(&shown.settings.shell.command);
        }
        self.themes = choices(&self.themes_dir);
        self.surface
            .set_icon(crate::icon::window(shown.appearance.look(shown.system)));
        self.surface.redraw();
    }

    pub fn window_event(&mut self, event: &WindowEvent, shown: &Shown<'_>) -> Outcome {
        if matches!(event, WindowEvent::CloseRequested) {
            return Outcome::Closed;
        }
        if self.surface.take(event) {
            return self.draw(shown);
        }
        Outcome::Nothing
    }

    fn draw(&mut self, shown: &Shown<'_>) -> Outcome {
        let path = self
            .store
            .path()
            .parent()
            .unwrap_or(self.store.path())
            .display()
            .to_string();
        let themes_dir = self.themes_dir.display().to_string();
        let files = Files {
            settings: self.store.path(),
            appearance: &self.appearance_path,
            themes: &self.themes_dir,
        };
        let mut changes = Vec::new();
        let (page, icon, shell) = (&mut self.page, self.icon.as_ref(), &mut self.shell);
        let (families, themes, problem) = (&self.families, &self.themes, self.problem.as_deref());
        self.surface.frame(|ui| {
            changes = pages::show(
                ui,
                page,
                View {
                    shown,
                    families,
                    themes,
                    themes_dir: &themes_dir,
                    files: &files,
                    icon,
                    shell,
                    problem,
                    path: &path,
                },
            );
        });
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
                self.surface.redraw();
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
                Change::Open(target) => open(target)?,
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

/// Show `target`, making a folder first if it is not there yet.
fn open(target: &Target) -> anyhow::Result<()> {
    if let Target::Folder(dir) = target {
        std::fs::create_dir_all(dir)?;
    }
    about::command(target).spawn()?;
    Ok(())
}

/// The icon as a texture, for the About page.
fn icon_texture(
    context: &egui::Context,
    look: winit::window::Theme,
) -> Option<egui::TextureHandle> {
    let (pixels, width, height) = crate::icon::rgba(crate::icon::large(look)).ok()?;
    let size = [usize::try_from(width).ok()?, usize::try_from(height).ok()?];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    Some(context.load_texture("icon", image, egui::TextureOptions::LINEAR))
}

/// The installed families a grid can be drawn with; empty when the
/// platform cannot list them.
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
