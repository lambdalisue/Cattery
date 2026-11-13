//! A window showing one session.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use felis_client_core::config::EffectiveConfig;
use felis_client_core::cursor_blink::BlinkClock;
use felis_client_core::{Offer, OutgoingFrame, ShadowScreen};
use felis_grid::image_layout::ImageLayout;
use felis_protocol::ImageId;
use felis_protocol::messages::{InputMsg, SessionInfo};
use felis_render_core::palette::Theme;
use felis_render_core::{ImageData, ImageSource};
use felis_render_wgpu::{Renderer, RendererConfig, SurfaceError};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::app::Event;
use crate::keys::{self, Command, Stroke};
use crate::settings::{self, Changes};
use crate::{daemon, grid, icon, inserted, session};

/// How long the daemon is given to end a session when its window closes.
const ENDING: Duration = Duration::from_secs(5);

/// What a window event asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    Command(Command),
}

pub struct Terminal {
    window: Arc<Window>,
    renderer: Renderer,
    shadow: ShadowScreen,
    session: SessionInfo,
    /// Dropped when the window goes, which lets the writer drain and end.
    outgoing: Option<UnboundedSender<OutgoingFrame>>,
    blink: BlinkClock,
    modifiers: ModifiersState,
    focused: bool,
    /// The key an input method inserted text for, whose release the session
    /// is not sent.
    inserted_for: Option<PhysicalKey>,
    /// Whether the program in the session has ended on its own.
    ended: bool,
}

impl Terminal {
    /// Open a window on a new session of the daemon at `socket`, started
    /// in `cwd` and drawn as `settings` say. Its session's events reach
    /// `proxy` named by the session's id.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        proxy: EventLoopProxy<Event>,
        socket: &Path,
        settings: &EffectiveConfig,
        look: winit::window::Theme,
        cwd: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let (session, outgoing) = runtime.block_on(async {
            let mut conn = daemon::connect(socket, &std::env::current_exe()?).await?;
            let session = session::create(&mut conn, &settings.shell, cwd, socket).await?;
            let id = session.id;
            let outgoing = session::pump(conn, move |event| {
                proxy.send_event(Event::Session(id, event)).is_ok()
            });
            anyhow::Ok((session, outgoing))
        })?;
        let drawn = settings::look(settings);
        // A surface is opaque or not for good once it is made.
        let translucent = drawn.theme.background_opacity.is_some();
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Cattery")
                    .with_window_icon(icon::window(look))
                    .with_transparent(translucent),
            )?,
        );
        let size = window.inner_size();
        let config = RendererConfig {
            font_size_physical_px: Some(physical(&window, settings::font_size(settings))),
            ..drawn
        };
        let colors = settings::reported_colors(&config);
        let renderer = runtime.block_on(Renderer::new_with_config(
            Arc::clone(&window),
            size.width,
            size.height,
            config,
            None,
        ))?;
        // winit hands keys to no input method until a window asks.
        window.set_ime_allowed(true);
        inserted::watch(&window);
        let mut terminal = Self {
            shadow: ShadowScreen::new(session.dims.rows, session.dims.cols),
            blink: BlinkClock::new(
                settings.cursor.blink,
                settings.cursor.blink_interval_ms,
                Instant::now(),
            ),
            window,
            renderer,
            session,
            outgoing: Some(outgoing),
            modifiers: ModifiersState::empty(),
            focused: true,
            inserted_for: None,
            ended: false,
        };
        terminal.send(OutgoingFrame::ordered(&colors));
        terminal.fit(size);
        Ok(terminal)
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Show the icon that goes with `look`.
    pub fn set_icon(&self, look: winit::window::Theme) {
        self.window.set_window_icon(icon::window(look));
    }

    pub fn session(&self) -> u128 {
        self.session.id
    }

    /// The directory the shell last reported being in, as it reported it.
    pub fn reported_cwd(&self) -> Option<&str> {
        self.shadow.cwd()
    }

    pub fn window_event(&mut self, event: WindowEvent, settings: &EffectiveConfig) -> Asked {
        match event {
            WindowEvent::CloseRequested => return Asked::Command(Command::CloseWindow),
            WindowEvent::Resized(size) => self.fit(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.renderer
                    .reload_font_size(physical(&self.window, settings::font_size(settings)));
                self.fit(self.window.inner_size());
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.renderer.set_window_focused(focused);
                self.show_cursor();
                self.redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(command) = keys::command(&event.logical_key, self.modifiers) {
                    // Both the press and the release stay out of the session.
                    return if event.state.is_pressed() && !event.repeat {
                        Asked::Command(command)
                    } else {
                        Asked::Nothing
                    };
                }
                self.type_key(&event);
            }
            WindowEvent::Ime(ime) => self.compose(&ime),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
        Asked::Nothing
    }

    /// Take what the session's daemon sent; whether the window stays open.
    pub fn session_event(&mut self, event: session::Event) -> anyhow::Result<bool> {
        match event {
            session::Event::Grid(msg) => {
                self.shadow.apply(&msg)?;
                self.redraw();
                Ok(true)
            }
            session::Event::Exited => {
                self.ended = true;
                Ok(false)
            }
            session::Event::Closed => Ok(false),
        }
    }

    /// Draw with `after` where it differs from `before`.
    pub fn restyle(&mut self, before: &EffectiveConfig, after: &EffectiveConfig) {
        let look = settings::look(after);
        let changes = Changes::between(
            &settings::look(before),
            &look,
            (settings::font_size(before), settings::font_size(after)),
        );
        self.renderer.set_theme(&Theme::from_config(&look.theme));
        let size = physical(&self.window, settings::font_size(after));
        if changes.faces {
            if let Err(err) = self.renderer.reload_font(
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
                self.renderer.reload_font_size(size);
            }
            if changes.features {
                self.renderer.reload_font_features(&look.font_features);
            }
        }
        if changes.cell_scale {
            self.renderer
                .set_cell_scale(look.cell_scale.unwrap_or([1.0, 1.0]));
        }
        if changes.refits() {
            self.fit(self.window.inner_size());
        }
        if changes.reported_colors {
            self.send(OutgoingFrame::ordered(&settings::reported_colors(&look)));
        }
        if before.cursor != after.cursor {
            let cursor = &after.cursor;
            self.blink
                .reconfigure(cursor.blink, cursor.blink_interval_ms, Instant::now());
            self.show_cursor();
        }
        self.redraw();
    }

    /// Blink the cursor if it is time; when to look again.
    pub fn blink(&mut self, now: Instant) -> Option<Instant> {
        let blinks = self.cursor_shown() && self.shadow.screen().cursor_blink();
        if self.blink.tick(now, blinks) {
            self.renderer.set_cursor_blink_visible(self.blink.visible());
            self.redraw();
        }
        self.blink.next_deadline(blinks)
    }

    /// Close the window, ending its session unless it ended on its own.
    pub fn close(mut self, runtime: &Runtime, socket: &Path) {
        self.outgoing = None;
        // Nothing reopens a session whose window closed, so one left
        // running would be out of reach until the daemon stops.
        if !self.ended {
            let id = self.session.id;
            let ended =
                runtime.block_on(async { tokio::time::timeout(ENDING, end(socket, id)).await? });
            if let Err(err) = ended {
                tracing::warn!(?err, id, "the session was left running");
            }
        }
    }

    fn type_key(&mut self, event: &winit::event::KeyEvent) {
        self.show_cursor();
        if event.state.is_pressed() {
            let inserted = inserted::take();
            if let Some(input) = keys::inserted_instead(inserted.as_deref(), event.text.as_deref())
            {
                self.inserted_for = Some(event.physical_key);
                self.send(OutgoingFrame::input(&input));
                return;
            }
        } else if self.inserted_for == Some(event.physical_key) {
            // The key was never pressed as far as the session knows.
            self.inserted_for = None;
            return;
        }
        self.send(OutgoingFrame::input(&keys::input(
            &Stroke::from(event),
            self.modifiers,
        )));
    }

    /// Resize the session to the cells the window now holds.
    fn fit(&mut self, size: PhysicalSize<u32>) {
        self.renderer.resize(size.width, size.height);
        let cell = self.renderer.cell_metrics();
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

    /// Show the cursor and start its blink over, as typing does.
    fn show_cursor(&mut self) {
        self.blink.reset(Instant::now());
        self.renderer.set_cursor_blink_visible(true);
    }

    fn redraw(&self) {
        self.window.request_redraw();
    }

    fn draw(&mut self) {
        // A screen still being sent whole would flash half drawn.
        if self.shadow.is_rehydrating() {
            return;
        }
        let drawn = self.renderer.render(
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
            Err(SurfaceError::Lost | SurfaceError::Outdated) => self.renderer.reconfigure_surface(),
            Err(err) => tracing::warn!(?err, "the frame was not drawn"),
        }
        // Glyphs rasterized during this frame are drawn on the next one.
        if self.renderer.take_glyph_repaint_request() {
            self.redraw();
        }
        self.place_input_method();
    }

    /// Tell the platform where the cursor is, so an input method shows
    /// its candidates beside it.
    fn place_input_method(&self) {
        let cell = self.renderer.cell_metrics();
        let cursor = self.shadow.screen().cursor();
        let (x, y) = self.renderer.content_origin_px();
        self.window.set_ime_cursor_area(
            PhysicalPosition::new(
                f64::from(x) + f64::from(cursor.col) * f64::from(cell.width),
                f64::from(y) + f64::from(cursor.row) * f64::from(cell.height),
            ),
            PhysicalSize::new(cell.width, cell.height),
        );
    }

    fn compose(&mut self, ime: &Ime) {
        if let Some(input) = keys::committed(ime) {
            self.show_cursor();
            self.send(OutgoingFrame::input(&input));
        }
        let cursor = self.shadow.screen().cursor();
        self.renderer
            .set_preedit(keys::composing(ime, (cursor.row, cursor.col)));
        self.redraw();
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
