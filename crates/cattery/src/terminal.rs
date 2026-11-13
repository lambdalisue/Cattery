//! A window showing one session.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use felis_client_core::config::EffectiveConfig;
use felis_client_core::config::Osc52Policy;
use felis_client_core::confirm::{ConfirmDecision, PendingConfirm, decide_confirm_key};
use felis_client_core::cursor_blink::BlinkClock;
use felis_client_core::viewport::{self, ViewportIntent};
use felis_client_core::{GridPos, Offer, OutgoingFrame, Selection, SelectionMode, ShadowScreen};
use felis_grid::ClipboardSelection;
use felis_grid::image_layout::ImageLayout;
use felis_protocol::ImageId;
use felis_protocol::messages::{
    InputMsg, Key, KeyEventKind, MouseAction, MouseButton, MouseEvent, SessionInfo,
};
use felis_render_core::overlay::ConfirmOverlay;
use felis_render_core::palette::Theme;
use felis_render_core::{ImageData, ImageSource, SelectionRange, layout};
use felis_render_wgpu::{Renderer, RendererConfig, SurfaceError};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::app::Event;
use crate::keys::{self, Command, Stroke};
use crate::select::{self, Clicks, Paste};
use crate::settings::{self, Changes, Zoom};
use crate::{clipboard, daemon, grid, icon, inserted, mouse, session, title};

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
    /// The protocol's minor version the daemon and Cattery agreed on.
    minor: u16,
    /// Whether the window means to show the live screen.
    intent: ViewportIntent,
    /// Where the pointer is, while it is over the window.
    pointer: Option<PhysicalPosition<f64>>,
    /// The buttons pressed while the program was told of them.
    grabbed: Vec<MouseButton>,
    clicks: Clicks,
    /// Whether the left button is held selecting.
    selecting: bool,
    /// Where a selection starts that the pointer has not yet drawn out.
    anchored: Option<Selection>,
    selection: Option<Selection>,
    /// What waits for the confirmation bar's answer.
    pending: Option<PendingConfirm>,
    /// Whether a program's writes to the clipboard reach the system's.
    osc_52: Osc52Policy,
    /// Trackpad pixels not yet a row.
    wheel_residue: f64,
    /// The rows a notch of the wheel scrolls, as the settings give.
    lines: f64,
    /// The key an input method inserted text for, whose release the session
    /// is not sent.
    inserted_for: Option<PhysicalKey>,
    /// The text size, in logical pixels, the window is drawn at: the
    /// settings' until it is zoomed.
    size: f32,
    /// The settings' `window.title_prefix`, which every title starts with.
    title_prefix: Option<String>,
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
        let (session, outgoing, minor) = runtime.block_on(async {
            let mut conn = daemon::connect(socket, &std::env::current_exe()?).await?;
            let session = session::create(&mut conn, &settings.shell, cwd, socket).await?;
            let minor = conn.effective_minor;
            let id = session.id;
            let outgoing = session::pump(conn, move |event| {
                proxy.send_event(Event::Session(id, event)).is_ok()
            });
            anyhow::Ok((session, outgoing, minor))
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
            minor,
            intent: ViewportIntent::default(),
            pointer: None,
            grabbed: Vec::new(),
            clicks: Clicks::default(),
            selecting: false,
            anchored: None,
            selection: None,
            pending: None,
            osc_52: settings.clipboard.osc_52,
            wheel_residue: 0.0,
            lines: settings.mouse.clamped_scroll_multiplier(),
            inserted_for: None,
            title_prefix: settings.window.title_prefix.clone(),
            size: settings::font_size(settings),
            ended: false,
        };
        terminal.retitle();
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

    pub fn window_event(&mut self, event: WindowEvent) -> Asked {
        match event {
            WindowEvent::CloseRequested => return Asked::Command(Command::CloseWindow),
            WindowEvent::Resized(size) => self.fit(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.renderer
                    .reload_font_size(physical(&self.window, self.size));
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
            WindowEvent::CursorMoved { position, .. } => self.point(position),
            WindowEvent::CursorLeft { .. } => self.pointer = None,
            WindowEvent::MouseInput { state, button, .. } => self.press(state, button),
            WindowEvent::MouseWheel { delta, .. } => self.turn_wheel(delta, self.lines),
            WindowEvent::DroppedFile(path) => self.paste(&select::dropped(&path)),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
        Asked::Nothing
    }

    /// Take what the session's daemon sent; whether the window stays open.
    pub fn session_event(&mut self, event: session::Event) -> anyhow::Result<bool> {
        match event {
            session::Event::Grid(msg) => {
                let before = (self.shadow.alt_screen(), self.shadow.viewport());
                self.shadow.apply(&msg)?;
                let after = (self.shadow.alt_screen(), self.shadow.viewport());
                if self.selection.is_some()
                    && select::invalidated((before.0, after.0), (before.1, after.1))
                {
                    self.select(None);
                }
                if self.shadow.take_title_dirty().is_some() {
                    self.retitle();
                }
                // A program's write to the clipboard reaches the system's only
                // where the settings let it, as in felis's own window.
                if let Some(write) = self.shadow.take_pending_clipboard_set()
                    && self.osc_52 == Osc52Policy::System
                    && write.selection.contains(ClipboardSelection::CLIPBOARD)
                {
                    clipboard::write(&String::from_utf8_lossy(&write.data));
                }
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
        self.osc_52 = after.clipboard.osc_52;
        self.lines = after.mouse.clamped_scroll_multiplier();
        if before.window.title_prefix != after.window.title_prefix {
            self.title_prefix.clone_from(&after.window.title_prefix);
            self.retitle();
        }
        let look = settings::look(after);
        let changes = Changes::between(
            &settings::look(before),
            &look,
            (settings::font_size(before), settings::font_size(after)),
        );
        // A size set anew drops the window's zoom, as felis's window does.
        if changes.size || changes.faces {
            self.size = settings::font_size(after);
        }
        self.renderer.set_theme(&Theme::from_config(&look.theme));
        let size = physical(&self.window, self.size);
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

    /// Make the window's text a step bigger or smaller, or as the settings
    /// `drawn` say again.
    pub fn zoom(&mut self, step: Zoom, drawn: &EffectiveConfig) {
        let size = settings::zoomed(self.size, step, settings::font_size(drawn));
        if (size - self.size).abs() < f32::EPSILON {
            return;
        }
        self.size = size;
        self.renderer
            .reload_font_size(physical(&self.window, self.size));
        self.fit(self.window.inner_size());
    }

    /// The selected text, if any is selected.
    pub fn copy(&self) -> Option<String> {
        self.selection
            .map(|selection| selection.extract_text(self.shadow.screen()))
            .filter(|text| !text.is_empty())
    }

    /// Paste `text` into the session, asking first where it would run
    /// commands the program did not ask to take as a paste.
    pub fn paste(&mut self, text: &str) {
        match select::paste(text, self.shadow.bracketed_paste()) {
            Paste::Send(input) => self.type_input(input),
            Paste::Confirm(InputMsg::Paste(bytes)) => {
                let pending = PendingConfirm::Paste(bytes);
                self.renderer.set_confirm_overlay(Some(ConfirmOverlay {
                    label: pending.label(),
                }));
                self.pending = Some(pending);
                self.redraw();
            }
            Paste::Confirm(_) => {}
            Paste::TooLarge => tracing::warn!(bytes = text.len(), "too much to paste"),
        }
    }

    /// Answer the confirmation bar with the key typed: y goes ahead, and
    /// any other key, Escape among them, does not.
    fn answer(&mut self, text: Option<&str>) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.renderer.set_confirm_overlay(None);
        if decide_confirm_key(text) == ConfirmDecision::Confirm
            && let PendingConfirm::Paste(bytes) = pending
        {
            self.type_input(InputMsg::Paste(bytes));
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

    fn retitle(&self) {
        self.window.set_title(&title::title(
            self.title_prefix.as_deref(),
            self.shadow.title(),
        ));
    }

    fn type_key(&mut self, event: &winit::event::KeyEvent) {
        self.show_cursor();
        if self.pending.is_some() {
            if event.state.is_pressed() {
                self.answer(event.text.as_deref());
            }
            return;
        }
        if event.state.is_pressed() {
            if let Some(direction) = keys::page(&event.logical_key, self.modifiers)
                && !self.shadow.alt_screen()
            {
                let half = i32::from(self.shadow.screen().rows() / 2).max(1);
                self.scroll(direction * half);
                return;
            }
            let inserted = inserted::take();
            if let Some(input) = keys::inserted_instead(inserted.as_deref(), event.text.as_deref())
            {
                self.inserted_for = Some(event.physical_key);
                self.type_input(input);
                return;
            }
        } else if self.inserted_for == Some(event.physical_key) {
            // The key was never pressed as far as the session knows.
            self.inserted_for = None;
            return;
        }
        self.type_input(keys::input(&Stroke::from(event), self.modifiers));
    }

    /// Send what was typed, showing the live screen first, as typing while
    /// reading history returns to the prompt in other terminals. A key's
    /// release, or a bare modifier, leaves the view where it is.
    fn type_input(&mut self, input: InputMsg) {
        let typed = match &input {
            InputMsg::Key(event) => {
                event.kind != KeyEventKind::Release
                    && (event.key != Key::Other || event.text.is_some())
            }
            _ => true,
        };
        if typed && !self.intent.is_live(self.shadow.viewport()) {
            self.input(InputMsg::Viewport {
                lines_from_bottom: 0,
                anchor: None,
            });
        }
        self.input(input);
    }

    /// Scroll the view `rows` into history, or out of it when negative.
    fn scroll(&mut self, rows: i32) {
        // The daemon keeps the alternate screen live; a step it ignores
        // would leave the window thinking it is reading history.
        if self.shadow.alt_screen() {
            return;
        }
        let max = self
            .shadow
            .viewport_max()
            .saturating_sub(u32::from(self.shadow.screen().rows()));
        if let Some(step) = viewport::step_request(
            rows,
            self.minor,
            self.shadow.viewport(),
            self.shadow.viewport_anchor(),
            max,
        ) {
            self.input(step);
        }
    }

    fn input(&mut self, input: InputMsg) {
        self.intent.note(&input);
        self.send(OutgoingFrame::input(&input));
    }

    /// The cell under the pointer, and its pixel for programs that ask in
    /// pixels.
    fn pointed(&self) -> Option<((u16, u16), (u16, u16))> {
        let position = self.pointer?;
        let cell = self.renderer.cell_metrics();
        let origin: [f32; 2] = self.renderer.content_origin_px().into();
        let screen = self.shadow.screen();
        let at = [position.x, position.y];
        Some((
            layout::cell_at(
                at,
                origin,
                [cell.width, cell.height],
                screen.rows(),
                screen.cols(),
            ),
            layout::report_pixel(at, origin),
        ))
    }

    fn report(&mut self, button: Option<MouseButton>, action: MouseAction) {
        let Some(((row, col), (px, py))) = self.pointed() else {
            return;
        };
        self.input(InputMsg::Mouse(MouseEvent {
            button,
            action,
            mods: mouse::mods(self.modifiers),
            x: col + 1,
            y: row + 1,
            px,
            py,
        }));
    }

    fn press(&mut self, state: ElementState, button: winit::event::MouseButton) {
        let Some(button) = mouse::button(button) else {
            return;
        };
        match state {
            ElementState::Pressed => {
                if mouse::to_program(self.shadow.mouse_protocol_active(), self.modifiers) {
                    // What the program does with the click is its own;
                    // a selection left showing would be of other text.
                    self.select(None);
                    self.grabbed.push(button);
                    self.report(Some(button), MouseAction::Press);
                } else if button == MouseButton::Left {
                    self.start_selecting();
                }
            }
            // A release follows its press: to the program if it was told of
            // the press, whatever has changed since.
            ElementState::Released => {
                if let Some(at) = self.grabbed.iter().position(|held| *held == button) {
                    self.grabbed.remove(at);
                    self.report(Some(button), MouseAction::Release);
                } else if button == MouseButton::Left {
                    self.selecting = false;
                }
            }
        }
    }

    /// Select from the cell under the pointer: from there for one click, the
    /// word for two, the line for three, a rectangle with Alt held.
    fn start_selecting(&mut self) {
        let Some(((row, col), _)) = self.pointed() else {
            return;
        };
        let cell = GridPos { row, col };
        let screen = self.shadow.screen();
        let clicks = self.clicks.record(Instant::now(), cell);
        let selection = match clicks {
            1 if self.modifiers.alt_key() => Some(Selection::new_rectangle(cell)),
            1 => Some(Selection::new(cell)),
            2 => Selection::word_at(screen, cell),
            _ => Selection::line_at(screen, row),
        };
        self.selecting = true;
        // A single click marks where a selection starts, and shows nothing
        // until the pointer moves.
        self.anchored = selection;
        self.select(selection.filter(|_| clicks > 1));
    }

    fn select(&mut self, selection: Option<Selection>) {
        self.selection = selection;
        self.renderer.set_selection(selection.map(|selection| {
            let (start, end) = selection.range();
            SelectionRange {
                start,
                end,
                rectangle: selection.mode() == SelectionMode::Rectangle,
            }
        }));
        self.redraw();
    }

    fn point(&mut self, position: PhysicalPosition<f64>) {
        let before = self.pointed().map(|(cell, _)| cell);
        self.pointer = Some(position);
        let after = self.pointed().map(|(cell, _)| cell);
        if before == after {
            return;
        }
        if self.selecting
            && let Some((row, col)) = after
            && let Some(mut selection) = self.selection.or(self.anchored)
        {
            selection.extend(GridPos { row, col });
            self.select(Some(selection));
            return;
        }
        // Programs are told of motion a cell at a time.
        if !self.shadow.mouse_protocol_active() {
            return;
        }
        match self.grabbed.last().copied() {
            Some(button) => self.report(Some(button), MouseAction::Drag),
            None if !self.modifiers.shift_key() => self.report(None, MouseAction::Motion),
            None => {}
        }
    }

    fn turn_wheel(&mut self, delta: MouseScrollDelta, lines: f64) {
        let cell = f64::from(self.renderer.cell_metrics().height);
        match mouse::wheel(
            delta,
            self.shadow.mouse_protocol_active(),
            self.shadow.alt_screen(),
            self.modifiers,
            lines,
            cell,
            &mut self.wheel_residue,
        ) {
            mouse::Wheel::Report(buttons) => {
                for button in buttons {
                    self.report(Some(button), MouseAction::Press);
                }
            }
            mouse::Wheel::Keys(keys) => self.input(InputMsg::KeyBytes(keys)),
            mouse::Wheel::Scroll(rows) => self.scroll(rows),
        }
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
            self.type_input(input);
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
