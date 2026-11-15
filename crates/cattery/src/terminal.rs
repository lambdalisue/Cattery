//! A window showing one session.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use felis_client_core::config::EffectiveConfig;
use felis_client_core::config::Osc52Policy;
use felis_client_core::confirm::{ConfirmDecision, PendingConfirm, decide_confirm_key};
use felis_client_core::cursor_blink::BlinkClock;
use felis_client_core::viewport::{self, ViewportIntent};
use felis_client_core::{
    ActivationTarget, GridPos, ImageShadow, OutgoingFrame, Selection, SelectionMode, ShadowScreen,
};
use felis_grid::ClipboardSelection;
use felis_grid::image_layout::ImageLayout;
use felis_protocol::ImageId;
use felis_protocol::messages::{
    GridMsg, ImageMsg, ImageTarget, InputMsg, Key, KeyEventKind, MouseAction, MouseButton,
    MouseEvent, PromptJump, SessionInfo, SourceRect, StreamId,
};
use felis_render_core::overlay::{ConfirmOverlay, SearchOverlay};
use felis_render_core::palette::Theme;
use felis_render_core::{ImageData, ImageSource, SelectionRange, layout};
use felis_render_wgpu::{
    FontFilesMode, MouseState, Renderer, RendererConfig, SurfaceError, TrailState,
};
use tokio::runtime::Runtime;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, ResizeDirection, UserAttentionType, Window, WindowId};

use crate::about::{self, Target};
use crate::app::Event;
use crate::chrome::Part;
use crate::keymap::Keymap;
use crate::keys::{self, Command, Stroke};
use crate::search::{self, Search, Step};
use crate::select::{self, Clicks, Paste};
use crate::session::Link;
use crate::settings::{self, Changes, Zoom};
use crate::{blur, chrome, clipboard, daemon, grid, icon, inserted, links, mouse, session, title};

/// Which session a window opens on.
#[derive(Debug, Clone, Copy)]
pub enum Start<'a> {
    /// A new one, started in this directory or else where Cattery was.
    New(Option<&'a Path>),
    /// One the daemon keeps.
    Attach(u128),
}

/// What a window event asks of the app.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Nothing,
    Command(Command),
    /// The window's menu, at the pointer.
    Menu,
    /// The app.s menu, below the title bar.s menu button.
    AppMenu,
}

pub struct Terminal {
    window: Arc<Window>,
    /// Whether the window is drawn translucent now.
    translucent: bool,
    /// The radius the window is blurred by.
    blurred: u32,
    /// How far down, in physical pixels, the grid is shown: below the
    /// title bar the window is drawn under.
    top: u32,
    /// The title bar Cattery draws, where it draws one.
    bar: Option<chrome::Bar>,
    /// When the title bar was last pressed to move the window, for a second
    /// press to maximize it.
    bar_pressed: Option<Instant>,
    /// Where the title bar is held, until the pointer moves off it far
    /// enough to move the window.
    bar_held: Option<PhysicalPosition<f64>>,
    renderer: Renderer,
    shadow: ShadowScreen,
    session: SessionInfo,
    /// Detached when the window goes.
    link: Option<Link>,
    /// The number the window.s session.s events arrive named by.
    link_number: u64,
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
    /// Whether the pointer shows that a click opens a link.
    over_link: bool,
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
    /// Whether Option composes characters rather than acting as Alt.
    option_composes: bool,
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
    /// What the window is called now.
    titled: String,
    search: Search,
    /// The stream the search's matches arrive on, while they do.
    searching: Option<StreamId>,
    /// The images the session's programs drew, and where they go on screen.
    images: ImageShadow,
    layout: ImageLayout,
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
        start: Start<'_>,
    ) -> anyhow::Result<Self> {
        let (session, link, minor, link_number) =
            Self::connect(runtime, proxy, socket, settings, start)?;
        let drawn = settings::look(settings);
        let translucent = drawn.theme.background_opacity.is_some();
        let window = Arc::new(
            event_loop.create_window(chrome::under_title_bar(
                Window::default_attributes()
                    .with_title("Cattery")
                    .with_window_icon(icon::window(look))
                    .with_transparent(true),
            ))?,
        );
        let size = window.inner_size();
        let config = RendererConfig {
            font_size_physical_px: Some(physical(&window, settings::font_size(settings))),
            font_files: crate::fonts::files(),
            font_files_mode: FontFilesMode::BeforeSystem,
            ..settings::made(&drawn)
        };
        let colors = settings::reported_colors(&config);
        let mut renderer = runtime.block_on(Renderer::new_with_config(
            Arc::clone(&window),
            size.width,
            size.height,
            config,
            None,
        ))?;
        renderer.set_theme(&Theme::from_config(&drawn.theme));
        // winit hands keys to no input method until a window asks.
        window.set_ime_allowed(true);
        inserted::watch(&window);
        let option_composes = option_composes(&window, settings);
        let mut terminal = Self {
            shadow: ShadowScreen::new(session.dims.rows, session.dims.cols),
            blink: BlinkClock::new(
                settings.cursor.blink,
                settings.cursor.blink_interval_ms,
                Instant::now(),
            ),
            window,
            translucent,
            blurred: 0,
            top: 0,
            bar: None,
            bar_pressed: None,
            bar_held: None,
            renderer,
            session,
            link: Some(link),
            link_number,
            modifiers: ModifiersState::empty(),
            focused: true,
            minor,
            intent: ViewportIntent::default(),
            pointer: None,
            grabbed: Vec::new(),
            over_link: false,
            clicks: Clicks::default(),
            selecting: false,
            anchored: None,
            selection: None,
            pending: None,
            osc_52: settings.clipboard.osc_52,
            option_composes,
            wheel_residue: 0.0,
            lines: settings.mouse.clamped_scroll_multiplier(),
            inserted_for: None,
            title_prefix: settings.window.title_prefix.clone(),
            titled: String::new(),
            size: settings::font_size(settings),
            search: Search::default(),
            searching: None,
            images: ImageShadow::new(),
            layout: ImageLayout::new(),
            ended: false,
        };
        terminal.retitle();
        terminal.send(OutgoingFrame::ordered(&colors));
        terminal.fit(size);
        Ok(terminal)
    }

    /// Attach to a session of the daemon at `socket` as `start` says, its
    /// events reaching `proxy` named by the number the link returned has.
    fn connect(
        runtime: &Runtime,
        proxy: EventLoopProxy<Event>,
        socket: &Path,
        settings: &EffectiveConfig,
        start: Start<'_>,
    ) -> anyhow::Result<(SessionInfo, Link, u16, u64)> {
        runtime.block_on(async {
            let mut conn = daemon::connect(socket, &std::env::current_exe()?).await?;
            let session = match start {
                Start::New(cwd) => session::create(&mut conn, &settings.shell, cwd, socket).await?,
                Start::Attach(id) => session::attach(&mut conn, id).await?,
            };
            let minor = conn.effective_minor;
            let number = session::link_number();
            let link = session::pump(conn, session.id, move |event| {
                proxy.send_event(Event::Session(number, event)).is_ok()
            });
            anyhow::Ok((session, link, minor, number))
        })
    }

    /// Show the session `id` in this window in place of its own, which is
    /// left running.
    pub fn switch(
        &mut self,
        runtime: &Runtime,
        proxy: EventLoopProxy<Event>,
        socket: &Path,
        settings: &EffectiveConfig,
        id: u128,
    ) -> anyhow::Result<()> {
        if id == self.session.id {
            return Ok(());
        }
        let (session, link, minor, link_number) =
            Self::connect(runtime, proxy, socket, settings, Start::Attach(id))?;
        if let Some(old) = self.link.replace(link) {
            runtime.block_on(old.detach());
        }
        self.link_number = link_number;
        self.minor = minor;
        self.shadow = ShadowScreen::new(session.dims.rows, session.dims.cols);
        self.session = session;
        self.images = ImageShadow::new();
        self.layout = ImageLayout::new();
        self.select(None);
        self.anchored = None;
        self.selecting = false;
        self.pending = None;
        self.renderer.set_confirm_overlay(None);
        self.grabbed.clear();
        self.searching = None;
        self.search = Search::default();
        self.renderer.set_search_overlay(None);
        self.intent = ViewportIntent::default();
        self.ended = false;
        self.titled.clear();
        let colors = settings::reported_colors(&settings::look(settings));
        self.send(OutgoingFrame::ordered(&colors));
        self.fit(self.window.inner_size());
        self.retitle();
        self.redraw();
        Ok(())
    }

    /// The number of the link the window's session's events arrive on.
    pub fn link_number(&self) -> u64 {
        self.link_number
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Where the app's menu opens in the window: below the title bar's
    /// menu button.
    pub fn below_menu_button(&self) -> PhysicalPosition<f64> {
        PhysicalPosition::new(0.0, f64::from(self.top))
    }

    /// Where the pointer is in the window, while it is over it.
    pub fn pointer(&self) -> Option<PhysicalPosition<f64>> {
        self.pointer
    }

    /// Where `at` in the window is on the screen, where the platform tells
    /// where the window is; Wayland does not.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn on_screen(&self, at: PhysicalPosition<f64>) -> Option<PhysicalPosition<i32>> {
        let window = self.window.inner_position().ok()?;
        #[expect(clippy::cast_possible_truncation, reason = "a pixel on a screen")]
        let (x, y) = (at.x.round() as i32, at.y.round() as i32);
        Some(PhysicalPosition::new(window.x + x, window.y + y))
    }

    /// Show the icon that goes with `look`.
    pub fn set_icon(&self, look: winit::window::Theme) {
        self.window.set_window_icon(icon::window(look));
    }

    /// Draw the window's frame in `frame`'s look, or the system's when none.
    pub fn set_frame(&self, frame: Option<winit::window::Theme>) {
        self.window.set_theme(frame);
    }

    pub fn session(&self) -> u128 {
        self.session.id
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// The directory the shell last reported being in, as it reported it.
    pub fn reported_cwd(&self) -> Option<&str> {
        self.shadow.cwd()
    }

    pub fn window_event(&mut self, event: WindowEvent, keymap: &Keymap) -> Asked {
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
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                self.hover();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(command) = keymap.command(&event.logical_key, self.modifiers) {
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
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                self.tell_pointer(None);
                if self.bar.is_some() {
                    self.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => return self.press(state, button),
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
                // A bell in a window out of sight asks for the user, as the
                // platform lets an app: the Dock bounces, the taskbar flashes.
                if matches!(msg, GridMsg::Attention { .. }) && !self.focused {
                    self.window
                        .request_user_attention(Some(UserAttentionType::Informational));
                }
                let before = (self.shadow.alt_screen(), self.shadow.viewport());
                self.shadow.apply(&msg)?;
                let after = (self.shadow.alt_screen(), self.shadow.viewport());
                if self.selection.is_some()
                    && select::invalidated((before.0, after.0), (before.1, after.1))
                {
                    self.select(None);
                }
                // The directory a shell reports marks no title as changed.
                _ = self.shadow.take_title_dirty();
                self.retitle();
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
            session::Event::Found(stream, msg) => {
                if self.searching == Some(stream) {
                    let first = self.search.current().is_none();
                    self.search.found(msg.into());
                    if first {
                        self.show_match();
                    }
                    self.redraw();
                }
                Ok(true)
            }
            session::Event::SearchEnded(stream, ended) => {
                if self.searching == Some(stream) {
                    self.searching = None;
                    self.search.ended(ended.err());
                    self.redraw();
                }
                Ok(true)
            }
            session::Event::Image(msg) => {
                self.images.apply(&msg)?;
                // A part of an image still arriving changes nothing drawn yet.
                let arriving = matches!(
                    msg,
                    ImageMsg::Chunk { .. }
                        | ImageMsg::Header {
                            target: ImageTarget::Frame { .. } | ImageTarget::Region { .. },
                            ..
                        }
                );
                if !arriving {
                    self.redraw();
                }
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
    /// Blur what shows through the window by `asked` points, as far as the
    /// window is translucent.
    pub fn blur(&mut self, asked: u32) {
        let radius = blur::radius(asked, self.translucent);
        if radius != self.blurred {
            self.blurred = radius;
            blur::apply(&self.window, radius);
        }
    }

    pub fn restyle(&mut self, before: &EffectiveConfig, after: &EffectiveConfig) {
        self.option_composes = option_composes(&self.window, after);
        self.osc_52 = after.clipboard.osc_52;
        self.lines = after.mouse.clamped_scroll_multiplier();
        if before.window.title_prefix != after.window.title_prefix {
            self.title_prefix.clone_from(&after.window.title_prefix);
            self.retitle();
        }
        let look = settings::look(after);
        self.translucent = look.theme.background_opacity.is_some();
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

    /// Show the prompt `jump` names, as the shell marked them.
    pub fn jump(&mut self, jump: PromptJump) {
        self.input(InputMsg::JumpPrompt { direction: jump });
    }

    /// Open the search bar, to search the session's history.
    pub fn find(&mut self) {
        self.stop_searching();
        self.search.begin();
        self.redraw();
    }

    /// Take a key while the search bar is open; whether it was the bar's.
    fn search_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if self.search.mode == search::Mode::Off || !event.state.is_pressed() {
            return self.search.mode != search::Mode::Off && !event.state.is_pressed();
        }
        let named = match &event.logical_key {
            WinitKey::Named(named) => Some(*named),
            _ => None,
        };
        match (&self.search.mode, named) {
            (_, Some(NamedKey::Escape)) => self.close_search(),
            (search::Mode::Typing, Some(NamedKey::Enter)) => {
                if self.search.query.is_empty() {
                    self.close_search();
                } else {
                    self.send_search();
                }
            }
            (search::Mode::Typing, Some(NamedKey::Backspace)) => {
                self.search.query.pop();
            }
            (search::Mode::Typing, _) => {
                let text = event.text.as_deref().unwrap_or_default();
                if text.chars().any(char::is_control) {
                    return true;
                }
                self.search.query.push_str(text);
            }
            (search::Mode::Shown, Some(NamedKey::Enter)) => self.step_match(Step::Older),
            (search::Mode::Shown, _) => match event.text.as_deref() {
                Some("n") => self.step_match(Step::Older),
                Some("N") => self.step_match(Step::Newer),
                // Any other key leaves the search and reaches the program.
                _ => {
                    self.close_search();
                    return false;
                }
            },
            (search::Mode::Off, _) => return false,
        }
        self.redraw();
        true
    }

    fn send_search(&mut self) {
        self.stop_searching();
        let Some(link) = self.link.as_ref() else {
            return;
        };
        match link.search(&self.search.query) {
            Ok(stream) => {
                self.searching = Some(stream);
                self.search.sent();
            }
            Err(err) => {
                self.search.sent();
                self.search.ended(Some(format!("{err:#}")));
            }
        }
    }

    fn step_match(&mut self, step: Step) {
        if self.search.step(step).is_some() {
            self.show_match();
        }
    }

    /// Bring the match chosen into view.
    fn show_match(&mut self) {
        let Some(hit) = self.search.current() else {
            return;
        };
        let rows = self.shadow.screen().rows();
        let max = self.shadow.viewport_max().saturating_sub(u32::from(rows));
        let (lines_from_bottom, anchor) =
            search::view_of(hit, self.shadow.viewport_anchor(), rows, max);
        self.input(InputMsg::Viewport {
            lines_from_bottom,
            anchor,
        });
    }

    fn close_search(&mut self) {
        self.stop_searching();
        self.search = Search::default();
        self.renderer.set_search_overlay(None);
        self.redraw();
    }

    fn stop_searching(&mut self) {
        if let (Some(stream), Some(link)) = (self.searching.take(), self.link.as_ref()) {
            link.cancel(stream);
        }
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

    /// Close the window, leaving its session running in the daemon, where
    /// another window can show it again.
    pub fn close(mut self, runtime: &Runtime) {
        if let Some(link) = self.link.take() {
            runtime.block_on(link.detach());
        }
    }

    fn retitle(&mut self) {
        let home = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_owned());
        let titled = title::title(
            self.title_prefix.as_deref(),
            self.shadow.cwd(),
            self.shadow.title(),
            home.as_deref(),
        );
        if titled != self.titled {
            self.window.set_title(&titled);
            self.titled = titled;
        }
    }

    fn type_key(&mut self, event: &winit::event::KeyEvent) {
        self.show_cursor();
        if self.search_key(event) {
            return;
        }
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
        let mods = keys::sent_mods(&event.logical_key, self.modifiers, self.option_composes);
        self.type_input(keys::input(&Stroke::from(event), mods));
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
        let origin = self.origin();
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

    fn press(&mut self, state: ElementState, button: winit::event::MouseButton) -> Asked {
        let Some(button) = mouse::button(button) else {
            return Asked::Nothing;
        };
        let reporting = self.shadow.mouse_protocol_active();
        match state {
            ElementState::Pressed => {
                if let Some(asked) = self.press_frame(button) {
                    return asked;
                }
                if button == MouseButton::Left && self.open_link() {
                    return Asked::Nothing;
                }
                if mouse::opens_menu(button, reporting, self.modifiers) {
                    return Asked::Menu;
                }
                if mouse::to_program(reporting, self.modifiers) {
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
                if button == MouseButton::Left {
                    self.bar_held = None;
                }
                if let Some(at) = self.grabbed.iter().position(|held| *held == button) {
                    self.grabbed.remove(at);
                    self.report(Some(button), MouseAction::Release);
                } else if button == MouseButton::Left {
                    self.selecting = false;
                }
            }
        }
        Asked::Nothing
    }

    /// Take a press of `button` on the frame Cattery draws: its edges, which
    /// resize the window, and its title bar. `None` for a press elsewhere.
    fn press_frame(&mut self, button: MouseButton) -> Option<Asked> {
        let bar = self.bar?;
        let position = self.pointer?;
        if let Some(direction) = self.resizes_from(position) {
            if button == MouseButton::Left
                && let Err(err) = self.window.drag_resize_window(direction)
            {
                tracing::warn!(?err, "the window was not resized");
            }
            return Some(Asked::Nothing);
        }
        let part = bar.part(position.x, position.y, self.window.inner_size().width)?;
        if button != MouseButton::Left {
            return Some(Asked::Nothing);
        }
        Some(match part {
            Part::Menu => Asked::AppMenu,
            Part::Minimize => {
                self.window.set_minimized(true);
                Asked::Nothing
            }
            Part::Maximize => {
                self.window.set_maximized(!self.window.is_maximized());
                Asked::Nothing
            }
            Part::Close => Asked::Command(Command::CloseWindow),
            Part::Drag => {
                let now = Instant::now();
                if self
                    .bar_pressed
                    .is_some_and(|at| now.duration_since(at) <= select::CLICK_INTERVAL)
                {
                    self.bar_pressed = None;
                    self.window.set_maximized(!self.window.is_maximized());
                } else {
                    // The window moves once the pointer does: a window
                    // manager that takes the pointer to move the window
                    // would take a second click with it.
                    self.bar_pressed = Some(now);
                    self.bar_held = Some(position);
                }
                Asked::Nothing
            }
        })
    }

    /// The edge the window is resized from at `position`, while Cattery
    /// draws its frame and the window is neither maximized nor full screen.
    fn resizes_from(&self, position: PhysicalPosition<f64>) -> Option<ResizeDirection> {
        if self.bar.is_none() || self.window.is_maximized() {
            return None;
        }
        let size = self.window.inner_size();
        let border = 6.0 * self.window.scale_factor();
        chrome::edge(position.x, position.y, (size.width, size.height), border)
    }

    /// Tell felis where the pointer is, for the title bar to mark the button
    /// under it.
    fn tell_pointer(&mut self, position: Option<PhysicalPosition<f64>>) {
        let size = self.window.inner_size();
        #[expect(clippy::cast_possible_truncation, reason = "a fraction of a window")]
        let pos_uv = position.map_or([-1.0, -1.0], |at| {
            [
                (at.x / f64::from(size.width.max(1))) as f32,
                (at.y / f64::from(size.height.max(1))) as f32,
            ]
        });
        self.renderer.set_mouse_state(MouseState {
            pos_uv,
            ..MouseState::default()
        });
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
        self.tell_pointer(Some(position));
        if let Some(held) = self.bar_held
            && (position.x - held.x).hypot(position.y - held.y) > 4.0 * self.window.scale_factor()
        {
            self.bar_held = None;
            if let Err(err) = self.window.drag_window() {
                tracing::warn!(?err, "the window was not moved");
            }
        }
        if let Some(bar) = self.bar {
            let size = self.window.inner_size();
            let edge = self
                .resizes_from(position)
                .map(CursorIcon::from)
                .unwrap_or_default();
            self.window.set_cursor(edge);
            if bar.part(position.x, position.y, size.width).is_some() {
                // The bar's buttons are marked under the pointer as it moves.
                self.pointer = Some(position);
                self.redraw();
                return;
            }
        }
        let before = self.pointed().map(|(cell, _)| cell);
        self.pointer = Some(position);
        let after = self.pointed().map(|(cell, _)| cell);
        if before == after {
            return;
        }
        self.hover();
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

    /// The link under the pointer, while the chord that opens links is held.
    fn link(&self) -> Option<ActivationTarget> {
        if !mouse::opens_links(self.modifiers) {
            return None;
        }
        let ((row, col), _) = self.pointed()?;
        links::at(&self.shadow, GridPos { row, col })
    }

    /// Open the link under the pointer, if there is one to open.
    fn open_link(&mut self) -> bool {
        let Some(link) = self.link() else {
            return false;
        };
        let target = Target::Url(link.as_str().to_owned());
        match about::command(&target).spawn() {
            // Reaped aside: a browser starting up cannot hold the window.
            Ok(mut opener) => drop(std::thread::spawn(move || opener.wait())),
            Err(err) => tracing::warn!(?err, "the link was not opened"),
        }
        true
    }

    /// Show a hand over a link that a click would open.
    fn hover(&mut self) {
        let over = self.link().is_some();
        if over != self.over_link {
            self.over_link = over;
            self.window.set_cursor(if over {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            });
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

    /// Size the window to hold `columns` and `rows` cells at the size its
    /// text is drawn at; an axis given none keeps its size.
    pub fn hold(&mut self, columns: Option<u32>, rows: Option<u32>) {
        if columns.is_none() && rows.is_none() {
            return;
        }
        let cell = self.renderer.cell_metrics();
        let top = chrome::title_bar(&self.window);
        let now = self.window.inner_size();
        let wanted = PhysicalSize::new(
            columns.map_or(now.width, |cols| grid::width_for(cols, cell.width)),
            rows.map_or(now.height, |rows| grid::height_for(rows, cell.height, top)),
        );
        // Where the platform resizes later, its Resized event fits the grid.
        if let Some(size) = self.window.request_inner_size(wanted) {
            self.fit(size);
        }
    }

    /// Resize the session to the cells the window now holds.
    fn fit(&mut self, size: PhysicalSize<u32>) {
        self.renderer.resize(size.width, size.height);
        let (top, bar) = (chrome::title_bar(&self.window), chrome::bar(&self.window));
        if (top, bar) != (self.top, self.bar) {
            (self.top, self.bar) = (top, bar);
            let button = bar.map_or(0, |bar| bar.button);
            if let Err(err) = self.renderer.set_post_stages(&[grid::margin(top, button)]) {
                tracing::warn!(?err, "the grid was not moved below the title bar");
            }
        }
        let cell = self.renderer.cell_metrics();
        let dims = grid::dims(
            size.width,
            size.height.saturating_sub(top),
            cell.width,
            cell.height,
        );
        self.shadow.local_resize(
            u16::try_from(dims.rows).unwrap_or(u16::MAX),
            u16::try_from(dims.cols).unwrap_or(u16::MAX),
        );
        self.send(OutgoingFrame::input(&InputMsg::Resize { dims }));
        self.redraw();
    }

    fn send(&self, frame: Result<OutgoingFrame, felis_transport::TransportError>) {
        let Some(link) = self.link.as_ref() else {
            return;
        };
        match frame {
            Ok(frame) => link.send(frame),
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
        if self.search.mode != search::Mode::Off {
            let screen = self.shadow.screen();
            self.renderer.set_search_overlay(Some(SearchOverlay {
                label: self.search.label(),
                visible_hits: self.search.spans(
                    self.shadow.viewport_anchor(),
                    self.shadow.viewport(),
                    screen.rows(),
                ),
            }));
        }
        let dirty = self.images.take_dirty_images();
        let cell = self.renderer.cell_metrics();
        #[expect(clippy::cast_precision_loss, reason = "a cell is a few dozen pixels")]
        let cell = (cell.width as f32, cell.height as f32);
        self.images.layout_into(
            &mut self.layout,
            self.shadow.screen(),
            cell,
            self.shadow.viewport(),
        );
        // Only felis's own cursor trail would tell the stage where the
        // cursor is, and Cattery shows none.
        let cursor = {
            let screen = self.shadow.screen();
            let at = screen.cursor();
            grid::cursor_cell(
                at.visible,
                self.shadow.viewport() != 0,
                (at.row, at.col),
                (screen.rows(), screen.cols()),
            )
            .map(|(row, col)| self.renderer.cursor_rect_uv(screen, row, col))
            .unwrap_or_default()
        };
        self.renderer.set_trail_state(TrailState {
            cursor_rect: cursor,
            prev_cursor_rect: cursor,
            ..TrailState::default()
        });
        let drawn = self.renderer.render(
            self.shadow.screen(),
            &dirty,
            &Images(&self.images),
            &self.layout,
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

    /// Where the grid's top-left corner is shown, in physical pixels.
    fn origin(&self) -> [f32; 2] {
        let (x, _) = self.renderer.content_origin_px();
        let inset = grid::inset(
            self.window.inner_size().width,
            self.renderer.cell_metrics().width,
        );
        // The margin stage shows the grid's top `top` pixels down, wherever
        // felis drew it.
        #[expect(clippy::cast_precision_loss, reason = "a few dozen pixels")]
        [x + inset as f32, self.top as f32]
    }

    /// Tell the platform where the cursor is, so an input method shows
    /// its candidates beside it.
    fn place_input_method(&self) {
        let cell = self.renderer.cell_metrics();
        let cursor = self.shadow.screen().cursor();
        let [x, y] = self.origin();
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

/// Whether Option composes characters in `window`, as on macOS unless the
/// settings make it Alt, which winit is then told of.
fn option_composes(window: &Window, settings: &EffectiveConfig) -> bool {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{OptionAsAlt, WindowExtMacOS as _};
        let as_alt = settings.window.macos_option_as_alt;
        window.set_option_as_alt(if as_alt {
            OptionAsAlt::Both
        } else {
            OptionAsAlt::None
        });
        !as_alt
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, settings);
        false
    }
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

/// The images a session's programs drew, as the renderer takes them: the
/// trait is the renderer's and the images client-core's.
struct Images<'a>(&'a ImageShadow);

impl ImageSource for Images<'_> {
    fn image_data(&self, id: ImageId) -> Option<ImageData<'_>> {
        let image = self.0.image(id).filter(|image| image.is_drawable())?;
        Some(ImageData {
            width: image.width,
            height: image.height,
            format: image.format,
            pixels: image.pixels(),
            generation: image.generation(),
        })
    }

    fn edits_since(&self, id: ImageId, generation: u64, out: &mut Vec<SourceRect>) -> bool {
        self.0.edits_since(id, generation, out)
    }
}
