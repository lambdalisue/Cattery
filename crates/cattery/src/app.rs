//! The app: its windows, each on a session of its own, the settings they
//! are drawn with, and the Preferences window.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use felis_client_core::config::EffectiveConfig;
use felis_client_core::config_watcher;
use tokio::runtime::Runtime;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Theme, WindowId};

use crate::appearance::{self, Appearance};
use crate::clipboard;
use crate::keys::Command;
use crate::menu::Menu;
use crate::preferences::{Outcome, Page, Preferences, Shown};
use crate::sessions::{Asked as SessionsAsked, Sessions};
use crate::terminal::{Asked, Start, Terminal};
use crate::{notify, place, roster, session, settings, themes};

/// What reaches the app from outside winit.
#[derive(Debug)]
pub enum Event {
    /// What the daemon sent about the session with this id.
    Session(u128, session::Event),
    /// The settings file or the appearance file was written.
    SettingsChanged,
    /// A command chosen from the menu.
    Command(Command),
    /// A program in a session posted a notification.
    Notified(notify::Posted),
}

/// Open a window on a new session of the daemon at `socket`, drawn and
/// started as the settings file at `settings_path` says, and run until
/// the last window closes.
pub fn run(socket: PathBuf, settings_path: Option<PathBuf>) -> anyhow::Result<()> {
    let settings = settings_path
        .as_deref()
        .map(load_at_start)
        .unwrap_or_default();
    let appearance = settings_path
        .as_deref()
        .map(|path| load_appearance_at_start(&appearance::path(path)))
        .unwrap_or_default();
    let runtime = Runtime::new()?;
    let event_loop = {
        let mut builder = EventLoop::<Event>::with_user_event();
        // Cattery's own menu takes the place of winit's.
        #[cfg(target_os = "macos")]
        winit::platform::macos::EventLoopBuilderExtMacOS::with_default_menu(&mut builder, false);
        builder.build()?
    };
    if let Some(path) = settings_path.as_deref() {
        for watched in [path.to_path_buf(), appearance::path(path)] {
            let proxy = event_loop.create_proxy();
            config_watcher::spawn(runtime.handle(), watched, move || {
                proxy.send_event(Event::SettingsChanged).is_ok()
            });
        }
    }
    let mut app = App {
        proxy: event_loop.create_proxy(),
        runtime,
        socket,
        settings_path,
        drawn: settings.clone(),
        settings,
        appearance,
        system: None,
        terminals: Vec::new(),
        focused: None,
        menu: None,
        sessions: None,
        listed_at: None,
        preferences: None,
        started: false,
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

/// An appearance file that does not read leaves the default appearance.
fn load_appearance_at_start(path: &Path) -> Appearance {
    appearance::load(path).unwrap_or_else(|err| {
        tracing::warn!(%err, "the appearance was not read; using the default");
        Appearance::default()
    })
}

struct App {
    runtime: Runtime,
    proxy: EventLoopProxy<Event>,
    socket: PathBuf,
    settings_path: Option<PathBuf>,
    /// The felis settings as the file says them.
    settings: EffectiveConfig,
    appearance: Appearance,
    /// The system's appearance, where the platform tells it.
    system: Option<Theme>,
    /// The settings with the theme in force under them: what the windows
    /// are drawn with.
    drawn: EffectiveConfig,
    terminals: Vec<Terminal>,
    /// The window last focused, which a window opened "here" opens from.
    focused: Option<WindowId>,
    menu: Option<Menu>,
    sessions: Option<Sessions>,
    /// When the Sessions window last listed the sessions.
    listed_at: Option<Instant>,
    preferences: Option<Preferences>,
    /// Whether the first window was opened.
    started: bool,
    failure: Option<anyhow::Error>,
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop, start: Start<'_>) -> anyhow::Result<()> {
        let terminal = Terminal::open(
            event_loop,
            &self.runtime,
            self.proxy.clone(),
            &self.socket,
            &self.drawn,
            self.appearance.look(self.system),
            start,
        )?;
        if let Some(menu) = self.menu.as_ref() {
            menu.attach(terminal.window());
        }
        self.focused = Some(terminal.id());
        self.terminals.push(terminal);
        self.list_sessions();
        Ok(())
    }

    fn command(&mut self, event_loop: &ActiveEventLoop, command: Command, from: Option<WindowId>) {
        let opened = match command {
            Command::Preferences => {
                self.open_preferences(event_loop);
                return;
            }
            Command::About => {
                self.open_preferences(event_loop);
                if let Some(preferences) = self.preferences.as_mut() {
                    preferences.show_page(Page::About);
                }
                return;
            }
            Command::CloseWindow => {
                if let Some(id) = from.or(self.focused) {
                    self.close(event_loop, id);
                }
                return;
            }
            Command::Sessions => {
                self.open_sessions(event_loop);
                return;
            }
            Command::Prompt(jump) => {
                if let Some(terminal) = self.terminal_mut(from) {
                    terminal.jump(jump);
                }
                return;
            }
            Command::Find => {
                if let Some(terminal) = self.terminal_mut(from) {
                    terminal.find();
                }
                return;
            }
            Command::Copy => {
                if let Some(text) = self.terminal_mut(from).and_then(|terminal| terminal.copy()) {
                    clipboard::write(&text);
                }
                return;
            }
            Command::Paste => {
                if let Some(text) = clipboard::read()
                    && let Some(terminal) = self.terminal_mut(from)
                {
                    terminal.paste(&text);
                }
                return;
            }
            Command::Zoom(step) => {
                let id = from.or(self.focused);
                let drawn = &self.drawn;
                if let Some(terminal) = self.terminals.iter_mut().find(|t| Some(t.id()) == id) {
                    terminal.zoom(step, drawn);
                }
                return;
            }
            // Leaving the loop closes every window, ending its session.
            Command::Quit => {
                event_loop.exit();
                return;
            }
            Command::NewWindow => self.open(event_loop, Start::New(place::home().as_deref())),
            Command::NewWindowHere => {
                let here = from
                    .or(self.focused)
                    .and_then(|id| self.terminal(id))
                    .and_then(|terminal| place::here(terminal.reported_cwd()))
                    .or_else(place::home);
                self.open(event_loop, Start::New(here.as_deref()))
            }
        };
        if let Err(err) = opened {
            tracing::warn!(?err, "the window did not open");
        }
    }

    /// The window `from`, or the one last focused.
    fn terminal_mut(&mut self, from: Option<WindowId>) -> Option<&mut Terminal> {
        let id = from.or(self.focused)?;
        self.terminals
            .iter_mut()
            .find(|terminal| terminal.id() == id)
    }

    fn terminal(&self, id: WindowId) -> Option<&Terminal> {
        self.terminals.iter().find(|terminal| terminal.id() == id)
    }

    /// Close the window `id`; the app ends with its last window.
    fn close(&mut self, event_loop: &ActiveEventLoop, id: WindowId) {
        if let Some(at) = self.terminals.iter().position(|t| t.id() == id) {
            self.terminals.swap_remove(at).close(&self.runtime);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        self.list_sessions();
        if self.terminals.is_empty() {
            event_loop.exit();
        }
    }

    /// Take the settings and appearance files as they now read, and redraw
    /// every window with what differs.
    fn reload(&mut self) {
        let Some(path) = self.settings_path.clone() else {
            return;
        };
        // Half-typed settings must not throw away the ones in use; felis's
        // reader has already said what is wrong.
        match settings::load(&path) {
            Ok(settings) => self.settings = settings,
            Err(err) => {
                tracing::warn!(%err, ?path, "the settings were not read; keeping those in use")
            }
        }
        match appearance::load(&appearance::path(&path)) {
            Ok(appearance) => self.appearance = appearance,
            Err(err) => tracing::warn!(%err, "the appearance was not read; keeping that in use"),
        }
        self.recolor();
    }

    /// Redraw every window with the settings and the theme in force now.
    fn recolor(&mut self) {
        let colored = self.colored();
        let before = std::mem::replace(&mut self.drawn, colored);
        let look = self.appearance.look(self.system);
        crate::icon::show_in_dock(look);
        for terminal in &mut self.terminals {
            terminal.restyle(&before, &self.drawn);
            terminal.set_icon(look);
        }
        if let Some(preferences) = self.preferences.as_mut() {
            preferences.reread(&Shown {
                settings: &self.settings,
                appearance: &self.appearance,
                drawn: &self.drawn,
                system: self.system,
            });
        }
    }

    /// The settings with the theme in force under them.
    fn colored(&self) -> EffectiveConfig {
        let name = self.appearance.in_force(self.system);
        match themes::colors(name, &self.themes_dir()) {
            Ok(colors) => colors.under(&self.settings),
            Err(err) => {
                tracing::warn!(%err, "the theme was not read; drawing without it");
                self.settings.clone()
            }
        }
    }

    fn themes_dir(&self) -> PathBuf {
        self.settings_path
            .as_deref()
            .map(appearance::themes_dir)
            .unwrap_or_default()
    }

    fn shown(&self) -> Shown<'_> {
        Shown {
            settings: &self.settings,
            appearance: &self.appearance,
            drawn: &self.drawn,
            system: self.system,
        }
    }

    /// Bring the Sessions window up, opening it if it is not open.
    fn open_sessions(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(sessions) = self.sessions.as_ref() {
            sessions.focus();
        } else {
            let over = self
                .focused
                .and_then(|id| self.terminal(id))
                .and_then(|terminal| {
                    let window = terminal.window();
                    Some((
                        window.outer_position().ok()?,
                        window.outer_size(),
                        window.scale_factor(),
                    ))
                });
            match Sessions::open(event_loop, &self.runtime, over) {
                Ok(sessions) => self.sessions = Some(sessions),
                Err(err) => tracing::warn!(?err, "the Sessions window did not open"),
            }
        }
        self.list_sessions();
    }

    /// Show the Sessions window, where it is open, what the daemon keeps now.
    fn list_sessions(&mut self) {
        let Some(sessions) = self.sessions.as_mut() else {
            return;
        };
        self.listed_at = Some(Instant::now());
        match self.runtime.block_on(session::list(&self.socket)) {
            Ok(listed) => sessions.show(roster::rows(&listed, place::home().as_deref())),
            Err(err) => tracing::warn!(?err, "the sessions were not listed"),
        }
    }

    fn sessions_asked(&mut self, event_loop: &ActiveEventLoop, asked: SessionsAsked) {
        match asked {
            SessionsAsked::Nothing => return,
            SessionsAsked::Closed => {
                self.sessions = None;
                return;
            }
            SessionsAsked::New => {
                self.sessions = None;
                if let Err(err) = self.open(event_loop, Start::New(place::home().as_deref())) {
                    tracing::warn!(?err, "the window did not open");
                }
                return;
            }
            SessionsAsked::Open(id) => {
                self.sessions = None;
                if let Some(terminal) = self.terminals.iter().find(|t| t.session() == id) {
                    terminal.window().focus_window();
                } else if let Err(err) = self.open(event_loop, Start::Attach(id)) {
                    tracing::warn!(?err, "the session was not opened");
                }
                return;
            }
            SessionsAsked::End(id) => {
                if let Err(err) = self.runtime.block_on(session::end(&self.socket, id)) {
                    tracing::warn!(?err, "the session was not ended");
                }
            }
        }
        self.list_sessions();
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
        let shown = self.shown();
        match Preferences::open(event_loop, &self.runtime, path, &shown) {
            Ok(preferences) => self.preferences = Some(preferences),
            Err(err) => tracing::warn!(?err, "the Preferences window did not open"),
        }
    }
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.started {
            return;
        }
        self.started = true;
        self.system = event_loop.system_theme();
        crate::icon::show_in_dock(self.appearance.look(self.system));
        self.drawn = self.colored();
        match Menu::install(self.proxy.clone()) {
            Ok(menu) => self.menu = Some(menu),
            Err(err) => tracing::warn!(?err, "the menu was not shown"),
        }
        if let Err(err) = self.open(event_loop, Start::New(None)) {
            self.failure = Some(err);
            event_loop.exit();
        }
        let (socket, proxy) = (self.socket.clone(), self.proxy.clone());
        self.runtime.spawn(async move {
            notify::watch(&socket, |posted| {
                proxy.send_event(Event::Notified(posted)).is_ok()
            })
            .await;
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(sessions) = self
            .sessions
            .as_mut()
            .filter(|sessions| sessions.id() == id)
        {
            let asked = sessions.window_event(&event);
            self.sessions_asked(event_loop, asked);
            return;
        }
        if let Some(preferences) = self
            .preferences
            .as_mut()
            .filter(|preferences| preferences.id() == id)
        {
            let shown = Shown {
                settings: &self.settings,
                appearance: &self.appearance,
                drawn: &self.drawn,
                system: self.system,
            };
            match preferences.window_event(&event, &shown) {
                Outcome::Nothing => {}
                Outcome::Saved => self.reload(),
                Outcome::Closed => self.preferences = None,
            }
            return;
        }
        if matches!(event, WindowEvent::Focused(true)) {
            self.focused = Some(id);
        }
        if let WindowEvent::ThemeChanged(theme) = event
            && self.system != Some(theme)
        {
            self.system = Some(theme);
            self.recolor();
        }
        let Some(terminal) = self.terminals.iter_mut().find(|t| t.id() == id) else {
            return;
        };
        match terminal.window_event(event) {
            Asked::Nothing => {}
            Asked::Command(command) => self.command(event_loop, command, Some(id)),
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Session(session, event) => {
                let Some(terminal) = self.terminals.iter_mut().find(|t| t.session() == session)
                else {
                    return;
                };
                let id = terminal.id();
                match terminal.session_event(event) {
                    Ok(true) => {}
                    Ok(false) => self.close(event_loop, id),
                    Err(err) => {
                        tracing::warn!(?err, session, "the session's screen could not be followed");
                        self.close(event_loop, id);
                    }
                }
            }
            Event::SettingsChanged => self.reload(),
            Event::Command(command) => self.command(event_loop, command, None),
            // The window in front already shows what its program said.
            Event::Notified(posted) => {
                if !self
                    .terminals
                    .iter()
                    .any(|terminal| terminal.is_focused() && terminal.session() == posted.session)
                {
                    notify::show(&posted);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let next = self
            .terminals
            .iter_mut()
            .filter_map(|terminal| terminal.blink(now))
            .min();
        // The Sessions window follows sessions that start and end on their own.
        let relist = self
            .listed_at
            .filter(|_| self.sessions.is_some())
            .map(|at| at + Duration::from_secs(2));
        if relist.is_some_and(|at| at <= now) {
            self.list_sessions();
        }
        let next = next
            .into_iter()
            .chain(
                self.listed_at
                    .filter(|_| self.sessions.is_some())
                    .map(|at| at + Duration::from_secs(2)),
            )
            .min();
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        for terminal in self.terminals.drain(..) {
            terminal.close(&self.runtime);
        }
    }
}
