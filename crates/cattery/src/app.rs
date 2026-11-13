//! A window showing one session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use felis_client_core::config::EffectiveConfig;
use felis_client_core::cursor_blink::BlinkClock;
use felis_client_core::{Offer, OutgoingFrame, ShadowScreen, config_watcher};
use felis_grid::image_layout::ImageLayout;
use felis_protocol::ImageId;
use felis_protocol::messages::{InputMsg, SessionInfo};
use felis_render_core::palette::Theme;
use felis_render_core::{ImageData, ImageSource};
use felis_render_wgpu::{Renderer, RendererConfig, SurfaceError};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::keys::{self, Stroke};
use crate::preferences::{Outcome, Preferences};
use crate::settings::{self, Changes};
use crate::{daemon, grid, session};

/// How long the daemon is given to end the session when its window closes.
const ENDING: Duration = Duration::from_secs(5);

/// What reaches the window from outside winit.
#[derive(Debug)]
pub enum Event {
    Session(session::Event),
    /// The settings file was written.
    SettingsChanged,
}

/// Open a window on a new session of the daemon at `socket`, drawn and
/// started as the settings file at `settings_path` says, and run until it
/// closes.
pub fn run(socket: PathBuf, settings_path: Option<PathBuf>) -> anyhow::Result<()> {
    let settings = settings_path
        .as_deref()
        .map(load_at_start)
        .unwrap_or_default();
    let runtime = Runtime::new()?;
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    let (session, outgoing) = runtime.block_on({
        let proxy = event_loop.create_proxy();
        let socket = socket.clone();
        let shell = settings.shell.clone();
        async move {
            let mut conn = daemon::connect(&socket, &std::env::current_exe()?).await?;
            let session = session::create(&mut conn, &shell, &socket).await?;
            let outgoing = session::pump(conn, move |event| {
                proxy.send_event(Event::Session(event)).is_ok()
            });
            anyhow::Ok((session, outgoing))
        }
    })?;
    if let Some(path) = settings_path.clone() {
        let proxy = event_loop.create_proxy();
        config_watcher::spawn(runtime.handle(), path, move || {
            proxy.send_event(Event::SettingsChanged).is_ok()
        });
    }
    let mut app = App {
        shadow: ShadowScreen::new(session.dims.rows, session.dims.cols),
        blink: BlinkClock::new(
            settings.cursor.blink,
            settings.cursor.blink_interval_ms,
            Instant::now(),
        ),
        runtime,
        socket,
        session,
        outgoing: Some(outgoing),
        settings_path,
        settings,
        window: None,
        renderer: None,
        modifiers: ModifiersState::empty(),
        focused: true,
        preferences: None,
        ended: false,
        failure: None,
    };
    event_loop.run_app(&mut app)?;
    app.failure.map_or(Ok(()), Err)
}

/// A settings file felis's reader refuses does not keep Cattery from
/// opening; felis's reader has already said what is wrong with it.
fn load_at_start(path: &Path) -> EffectiveConfig {
    settings::load(path).unwrap_or_else(|err| {
        tracing::warn!(%err, ?path, "the settings were not read; using the defaults");
        EffectiveConfig::default()
    })
}

struct App {
    runtime: Runtime,
    socket: PathBuf,
    session: SessionInfo,
    /// Dropped when the window goes, which lets the writer drain and end.
    outgoing: Option<UnboundedSender<OutgoingFrame>>,
    settings_path: Option<PathBuf>,
    settings: EffectiveConfig,
    shadow: ShadowScreen,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    blink: BlinkClock,
    modifiers: ModifiersState,
    focused: bool,
    preferences: Option<Preferences>,
    /// Whether the program in the session has ended on its own.
    ended: bool,
    failure: Option<anyhow::Error>,
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let look = settings::look(&self.settings);
        // A surface is opaque or not for good once it is made.
        let translucent = look.theme.background_opacity.is_some();
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Cattery")
                    .with_transparent(translucent),
            )?,
        );
        let size = window.inner_size();
        let config = RendererConfig {
            font_size_physical_px: Some(physical(&window, settings::font_size(&self.settings))),
            ..look
        };
        let colors = settings::reported_colors(&config);
        let renderer = self.runtime.block_on(Renderer::new_with_config(
            Arc::clone(&window),
            size.width,
            size.height,
            config,
            None,
        ))?;
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.send(OutgoingFrame::ordered(&colors));
        self.fit(size);
        Ok(())
    }

    /// Take the settings file as it now reads, and redo what differs.
    fn reload(&mut self) {
        let Some(path) = self.settings_path.as_deref() else {
            return;
        };
        let after = match settings::load(path) {
            Ok(after) => after,
            // Half-typed settings must not throw away the ones in use;
            // felis's reader has already said what is wrong.
            Err(err) => {
                tracing::warn!(%err, ?path, "the settings were not read; keeping those in use");
                return;
            }
        };
        let before = std::mem::replace(&mut self.settings, after);
        let look = settings::look(&self.settings);
        let changes = Changes::between(
            &settings::look(&before),
            &look,
            (
                settings::font_size(&before),
                settings::font_size(&self.settings),
            ),
        );
        if let (Some(window), Some(renderer)) = (self.window.clone(), self.renderer.as_mut()) {
            redraw_with(renderer, &window, &look, &changes, &self.settings);
            if changes.refits() {
                self.fit(window.inner_size());
            }
            if look.theme.background_opacity.is_some() != window_is_translucent(&before) {
                tracing::info!("a change between opaque and translucent applies to new windows");
            }
        }
        if changes.reported_colors {
            self.send(OutgoingFrame::ordered(&settings::reported_colors(&look)));
        }
        if before.cursor != self.settings.cursor {
            let cursor = &self.settings.cursor;
            self.blink
                .reconfigure(cursor.blink, cursor.blink_interval_ms, Instant::now());
            self.show_cursor();
        }
        self.redraw();
    }

    /// Resize the session to the cells the window now holds.
    fn fit(&mut self, size: PhysicalSize<u32>) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.resize(size.width, size.height);
        let cell = renderer.cell_metrics();
        let dims = grid::dims(size.width, size.height, cell.width, cell.height);
        self.shadow.local_resize(
            u16::try_from(dims.rows).unwrap_or(u16::MAX),
            u16::try_from(dims.cols).unwrap_or(u16::MAX),
        );
        self.send(OutgoingFrame::input(&InputMsg::Resize { dims }));
        self.redraw();
    }

    fn send(&self, frame: Result<OutgoingFrame, felis_transport::TransportError>) {
        let Some(outgoing) = self.outgoing.as_ref() else {
            return;
        };
        match frame {
            // A closed queue means the connection ended, which the reader
            // reports on its own.
            Ok(frame) => _ = outgoing.send(frame),
            Err(err) => tracing::warn!(?err, "a message to the daemon could not be encoded"),
        }
    }

    /// Bring the Preferences window up, opening it if it is not open.
    fn open_preferences(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(preferences) = self.preferences.as_ref() {
            preferences.focus();
            return;
        }
        let Some(path) = self.settings_path.clone() else {
            tracing::warn!("there is no settings file to change: no home directory was found");
            return;
        };
        match Preferences::open(event_loop, &self.runtime, path, &self.settings) {
            Ok(preferences) => self.preferences = Some(preferences),
            Err(err) => tracing::warn!(?err, "the Preferences window did not open"),
        }
    }

    /// Show the cursor and start its blink over, as typing does.
    fn show_cursor(&mut self) {
        self.blink.reset(Instant::now());
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_cursor_blink_visible(true);
        }
    }

    fn redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn draw(&mut self) {
        // A screen still being sent whole would flash half drawn.
        if self.shadow.is_rehydrating() {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let drawn = renderer.render(
            self.shadow.screen(),
            &[],
            &NoImages,
            &ImageLayout::new(),
            self.shadow.viewport(),
            self.shadow.viewport_max(),
        );
        self.shadow.clear_damage();
        match drawn {
            Ok(()) => {}
            Err(SurfaceError::Lost | SurfaceError::Outdated) => renderer.reconfigure_surface(),
            Err(err) => tracing::warn!(?err, "the frame was not drawn"),
        }
        // Glyphs rasterized during this frame are drawn on the next one.
        if renderer.take_glyph_repaint_request() {
            self.redraw();
        }
    }

    /// Whether the cursor is on screen in a focused window, where blinking
    /// it means something.
    fn cursor_shown(&self) -> bool {
        let screen = self.shadow.screen();
        let cursor = screen.cursor();
        self.focused
            && cursor.visible
            && self.shadow.viewport() == 0
            && cursor.row < screen.rows()
            && cursor.col < screen.cols()
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        self.failure.get_or_insert(err);
        event_loop.exit();
    }
}

fn redraw_with(
    renderer: &mut Renderer,
    window: &Window,
    look: &RendererConfig,
    changes: &Changes,
    settings: &EffectiveConfig,
) {
    renderer.set_theme(&Theme::from_config(&look.theme));
    let size = physical(window, settings::font_size(settings));
    if changes.faces {
        if let Err(err) = renderer.reload_font(
            look.font_family.as_deref(),
            Some(size),
            &look.font_features,
            &look.font_fallbacks,
            &look.style_faces(),
        ) {
            tracing::warn!(?err, "the font was not changed");
        }
    } else {
        if changes.size {
            renderer.reload_font_size(size);
        }
        if changes.features {
            renderer.reload_font_features(&look.font_features);
        }
    }
    if changes.cell_scale {
        renderer.set_cell_scale(look.cell_scale.unwrap_or([1.0, 1.0]));
    }
}

fn window_is_translucent(settings: &EffectiveConfig) -> bool {
    settings::look(settings).theme.background_opacity.is_some()
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(err) = self.open(event_loop)
        {
            self.fail(event_loop, err);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(preferences) = self
            .preferences
            .as_mut()
            .filter(|preferences| preferences.id() == id)
        {
            match preferences.window_event(&event, &self.settings) {
                Outcome::Nothing => {}
                Outcome::Saved => self.reload(),
                Outcome::Closed => self.preferences = None,
            }
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.fit(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(window), Some(renderer)) =
                    (self.window.clone(), self.renderer.as_mut())
                {
                    renderer
                        .reload_font_size(physical(&window, settings::font_size(&self.settings)));
                    self.fit(window.inner_size());
                }
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_window_focused(focused);
                }
                self.show_cursor();
                self.redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. }
                if keys::opens_preferences(&event.logical_key, self.modifiers) =>
            {
                // Both the press and the release stay out of the session.
                if event.state.is_pressed() && !event.repeat {
                    self.open_preferences(event_loop);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.show_cursor();
                self.send(OutgoingFrame::input(&keys::input(
                    &Stroke::from(&event),
                    self.modifiers,
                )));
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Session(session::Event::Grid(msg)) => match self.shadow.apply(&msg) {
                Ok(()) => self.redraw(),
                Err(err) => self.fail(event_loop, err.into()),
            },
            Event::Session(session::Event::Exited) => {
                self.ended = true;
                event_loop.exit();
            }
            Event::Session(session::Event::Closed) => event_loop.exit(),
            Event::SettingsChanged => {
                self.reload();
                if let Some(preferences) = self.preferences.as_mut() {
                    preferences.reread(&self.settings);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let blinks = self.cursor_shown() && self.shadow.screen().cursor_blink();
        if self.blink.tick(Instant::now(), blinks) {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.set_cursor_blink_visible(self.blink.visible());
            }
            self.redraw();
        }
        event_loop.set_control_flow(
            self.blink
                .next_deadline(blinks)
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.outgoing = None;
        // Nothing reopens a session whose window closed, so one left
        // running would be out of reach until the daemon stops.
        if !self.ended {
            let ending = end(&self.socket, self.session.id);
            if let Err(err) = self
                .runtime
                .block_on(async { tokio::time::timeout(ENDING, ending).await? })
            {
                tracing::warn!(?err, id = self.session.id, "the session was left running");
            }
        }
    }
}

async fn end(socket: &Path, id: u128) -> anyhow::Result<()> {
    let mut ops = felis_client_core::connect(socket, Offer::ops()).await?;
    ops.destroy_session(format!("{id:032x}")).await?;
    Ok(())
}

/// `logical` font pixels as `window`'s display draws them.
fn physical(window: &Window, logical: f32) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a display's scale factor is a small number"
    )]
    let scale = window.scale_factor() as f32;
    logical * scale
}

/// Images are not shown yet.
struct NoImages;

impl ImageSource for NoImages {
    fn image_data(&self, _: ImageId) -> Option<ImageData<'_>> {
        None
    }
}
