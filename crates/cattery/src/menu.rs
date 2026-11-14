//! Cattery's menus: the menu bar on macOS, a window's menu where it is
//! right-clicked, and on Linux and Windows the app's menu behind the menu
//! button of the title bar Cattery draws. The chords in
//! [`crate::keys::command`] do the same on every platform.

use winit::dpi::PhysicalPosition;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

use felis_protocol::messages::PromptJump;

use crate::app::Event;
use crate::keymap::Keymap;
use crate::keys::Command;
use crate::settings::Zoom;

const COMMANDS: [(Command, &str); 23] = [
    (Command::RestartDaemon, "restart-daemon"),
    (Command::CloseAllWindows, "close-all-windows"),
    (Command::Reload, "reload"),
    (Command::Help, "help"),
    (Command::ReportIssue, "report-issue"),
    (Command::Prompt(PromptJump::Previous), "previous-prompt"),
    (Command::Prompt(PromptJump::Next), "next-prompt"),
    (Command::Find, "find"),
    (Command::Palette, "palette"),
    (Command::Copy, "copy"),
    (Command::Paste, "paste"),
    (Command::Zoom(Zoom::In), "zoom-in"),
    (Command::Zoom(Zoom::Out), "zoom-out"),
    (Command::Zoom(Zoom::Reset), "zoom-reset"),
    (Command::About, "about"),
    (Command::Preferences, "preferences"),
    (Command::NewWindow, "new-window"),
    (Command::NewWindowHere, "new-window-here"),
    (Command::CloseWindow, "close-window"),
    (Command::Quit, "quit"),
    (Command::NextSession, "next-session"),
    (Command::PreviousSession, "previous-session"),
    (Command::EndSession, "end-session"),
];

const SHOW_SESSION: &str = "show-session:";
const END_SESSION: &str = "end-session:";

/// The id a menu item for `command` carries.
pub fn id(command: Command) -> String {
    match command {
        Command::ShowSession(session) => format!("{SHOW_SESSION}{session:x}"),
        Command::EndSessionOf(session) => format!("{END_SESSION}{session:x}"),
        _ => COMMANDS
            .iter()
            .find(|(each, _)| *each == command)
            .map_or_else(String::new, |(_, id)| (*id).to_owned()),
    }
}

/// The command the menu item `id` stands for.
pub fn command(id: &str) -> Option<Command> {
    let session = |prefix: &str| {
        id.strip_prefix(prefix)
            .and_then(|hex| u128::from_str_radix(hex, 16).ok())
    };
    if let Some(session) = session(SHOW_SESSION) {
        return Some(Command::ShowSession(session));
    }
    if let Some(session) = session(END_SESSION) {
        return Some(Command::EndSessionOf(session));
    }
    COMMANDS
        .iter()
        .find(|(_, each)| *each == id)
        .map(|(command, _)| *command)
}

/// An entry of a menu drawn alike on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Item(Command, &'static str),
    Separator,
    /// The sessions the daemon keeps, each shown or ended from the menu.
    Sessions,
}

/// A session as a menu lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: u128,
    pub title: String,
}

/// A line of a menu, with the sessions in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Item(Command, String),
    Separator,
    /// A session, shown in the window when it is chosen, or ended.
    Session(Listed),
}

/// What right-clicking a window offers.
pub const WINDOW_MENU: &[Entry] = &[
    Entry::Item(Command::Copy, "Copy"),
    Entry::Item(Command::Paste, "Paste"),
    Entry::Separator,
    Entry::Item(Command::Find, "Find…"),
    Entry::Item(Command::Palette, "Command Palette…"),
    Entry::Separator,
    Entry::Item(Command::NewWindow, "New Window"),
    Entry::Item(Command::NewWindowHere, "New Window Here"),
    Entry::Separator,
    Entry::Item(Command::Preferences, "Preferences…"),
];

/// What right-clicking Cattery's icon in the Dock offers. Nothing that acts
/// on the window in front, as from the Dock which window that is is not
/// seen, nor the palette, which is reached from any window.
pub const DOCK_MENU: &[Entry] = &[Entry::Item(Command::NewWindow, "New Window")];

/// What the menu button of the title bar Cattery draws offers: all the menu
/// bar does on macOS.
pub const APP_MENU: &[Entry] = &[
    Entry::Item(Command::NewWindow, "New Window"),
    Entry::Item(Command::NewWindowHere, "New Window Here"),
    Entry::Item(Command::Palette, "Command Palette…"),
    Entry::Separator,
    Entry::Item(Command::NextSession, "Next Session"),
    Entry::Item(Command::PreviousSession, "Previous Session"),
    Entry::Separator,
    Entry::Sessions,
    Entry::Separator,
    Entry::Item(Command::Copy, "Copy"),
    Entry::Item(Command::Paste, "Paste"),
    Entry::Item(Command::Find, "Find…"),
    Entry::Separator,
    Entry::Item(Command::Zoom(Zoom::In), "Bigger"),
    Entry::Item(Command::Zoom(Zoom::Out), "Smaller"),
    Entry::Item(Command::Zoom(Zoom::Reset), "Default Size"),
    Entry::Separator,
    Entry::Item(Command::Prompt(PromptJump::Previous), "Previous Prompt"),
    Entry::Item(Command::Prompt(PromptJump::Next), "Next Prompt"),
    Entry::Separator,
    Entry::Item(Command::Preferences, "Preferences…"),
    Entry::Item(Command::Reload, "Reload Configuration"),
    Entry::Item(Command::RestartDaemon, "Restart Daemon…"),
    Entry::Separator,
    Entry::Item(Command::Help, "Cattery Help"),
    Entry::Item(Command::ReportIssue, "Report an Issue…"),
    Entry::Item(Command::About, "About Cattery"),
    Entry::Separator,
    Entry::Item(Command::CloseWindow, "Close Window"),
    Entry::Item(Command::EndSession, "Close Window and End Session"),
    Entry::Item(Command::CloseAllWindows, "Close All Windows"),
    Entry::Item(Command::Quit, "Quit"),
];

/// The lines of a menu of `entries` with `sessions` in place of
/// [`Entry::Sessions`], a separator left out where nothing is between it
/// and the next.
pub fn lines(entries: &[Entry], sessions: &[Listed]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for entry in entries {
        match entry {
            Entry::Item(command, label) => lines.push(Line::Item(*command, (*label).to_owned())),
            Entry::Separator => {
                if lines.last().is_some_and(|last| *last != Line::Separator) {
                    lines.push(Line::Separator);
                }
            }
            Entry::Sessions => lines.extend(sessions.iter().cloned().map(Line::Session)),
        }
    }
    if lines.last() == Some(&Line::Separator) {
        lines.pop();
    }
    lines
}

/// Which of a window's menus to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// What right-clicking the window offers.
    Window,
    /// What the title bar's menu button offers.
    App,
}

impl Which {
    pub const fn entries(self) -> &'static [Entry] {
        match self {
            Self::Window => WINDOW_MENU,
            Self::App => APP_MENU,
        }
    }
}

/// The app's menus, while they are shown.
pub struct Menu {
    #[cfg(target_os = "macos")]
    _menu: muda::Menu,
    /// The menu bar's Sessions menu, which lists the sessions.
    #[cfg(target_os = "macos")]
    sessions: muda::Submenu,
    /// What right-clicking a window offers, as the platform draws a menu.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    window_menu: muda::Menu,
    /// The keys the menu bar shows and answers.
    #[cfg(target_os = "macos")]
    keymap: Keymap,
    /// What right-clicking the icon in the Dock offers.
    #[cfg(target_os = "macos")]
    _dock_menu: muda::Menu,
}

impl Menu {
    /// Show the menu, sending what is chosen in it, and on macOS a click on
    /// the icon in the Dock, to `proxy`.
    pub fn install(proxy: EventLoopProxy<Event>, keymap: &Keymap) -> anyhow::Result<Self> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            #[cfg(target_os = "macos")]
            let dock = proxy.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                if let Some(command) = command(event.id().as_ref()) {
                    _ = proxy.send_event(Event::Command(command));
                }
            }));
            #[cfg(target_os = "macos")]
            let (menu, sessions) = bar(keymap)?;
            #[cfg(not(target_os = "macos"))]
            let _ = keymap;
            #[cfg(target_os = "macos")]
            let dock_menu = platform::menu(&lines(DOCK_MENU, &[]))?;
            #[cfg(target_os = "macos")]
            answer_dock(&dock_menu, dock);
            Ok(Self {
                #[cfg(target_os = "macos")]
                _menu: menu,
                #[cfg(target_os = "macos")]
                sessions,
                window_menu: platform::menu(&lines(WINDOW_MENU, &[]))?,
                #[cfg(target_os = "macos")]
                keymap: keymap.clone(),
                #[cfg(target_os = "macos")]
                _dock_menu: dock_menu,
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (proxy, keymap);
            Ok(Self {})
        }
    }

    /// Show and answer the keys of `keymap` in the menu bar from now on;
    /// the sessions are to be listed again after.
    pub fn rekey(&mut self, keymap: &Keymap) {
        #[cfg(target_os = "macos")]
        if *keymap != self.keymap {
            match bar(keymap) {
                Ok((menu, sessions)) => {
                    self._menu = menu;
                    self.sessions = sessions;
                    self.keymap = keymap.clone();
                }
                Err(err) => tracing::warn!(?err, "the menu bar was not made anew"),
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = keymap;
    }

    /// List `sessions` where the menu bar does.
    pub fn list(&self, sessions: &[Listed]) {
        #[cfg(target_os = "macos")]
        if let Err(err) = platform::list(&self.sessions, sessions, &self.keymap) {
            tracing::warn!(?err, "the sessions were not listed in the menu bar");
        }
        #[cfg(not(target_os = "macos"))]
        let _ = sessions;
    }

    /// Show the menu `which` of `window`, with `sessions` in it, at `at` in
    /// the window or else at the pointer, until an item is chosen or the
    /// menu is dismissed.
    pub fn pop_up(
        &self,
        window: &Window,
        which: Which,
        sessions: &[Listed],
        at: Option<PhysicalPosition<f64>>,
    ) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            use muda::ContextMenu as _;
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            // The app's menu lists the sessions as they are now, so it is
            // made each time; the window's menu holds none.
            let app;
            let menu = match which {
                Which::Window => &self.window_menu,
                Which::App => match platform::menu(&lines(APP_MENU, sessions)) {
                    Ok(made) => {
                        app = made;
                        &app
                    }
                    Err(err) => {
                        tracing::warn!(?err, "the menu was not made");
                        return;
                    }
                },
            };
            let at = at.map(|at| muda::dpi::PhysicalPosition::new(at.x, at.y).into());
            let Ok(handle) = window.window_handle() else {
                return;
            };
            match handle.as_raw() {
                // SAFETY: the view is winit's, alive while `window` is.
                #[cfg(target_os = "macos")]
                RawWindowHandle::AppKit(handle) => unsafe {
                    menu.show_context_menu_for_nsview(handle.ns_view.as_ptr(), at);
                },
                // SAFETY: the window is winit's, alive while `window` is.
                #[cfg(target_os = "windows")]
                RawWindowHandle::Win32(handle) => unsafe {
                    menu.show_context_menu_for_hwnd(handle.hwnd.get(), at);
                },
                _ => {}
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = (window, which, sessions, at);
    }
}

/// Have AppKit show `menu`, which outlives the app's run, when Cattery's
/// icon in the Dock is right-clicked, and tell `proxy` when the icon is
/// clicked.
#[cfg(target_os = "macos")]
fn answer_dock(menu: &muda::Menu, proxy: EventLoopProxy<Event>) {
    use std::ffi::CString;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::sync::{Mutex, OnceLock};

    use muda::ContextMenu as _;
    use objc2::encode::Encode as _;
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::{class, msg_send, sel};

    type DockMenu = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> *mut AnyObject;
    type Reopen = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, Bool) -> Bool;

    static DOCK: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());
    static PROXY: OnceLock<Mutex<EventLoopProxy<Event>>> = OnceLock::new();

    unsafe extern "C-unwind" fn dock_menu(_: &AnyObject, _: Sel, _: &AnyObject) -> *mut AnyObject {
        DOCK.load(Ordering::Relaxed)
    }

    // AppKit still brings forward or restores a window there is; the app
    // opens one only where there is none.
    unsafe extern "C-unwind" fn reopen(_: &AnyObject, _: Sel, _: &AnyObject, _: Bool) -> Bool {
        if let Some(proxy) = PROXY.get().and_then(|proxy| proxy.lock().ok()) {
            _ = proxy.send_event(Event::Reopen);
        }
        Bool::YES
    }

    DOCK.store(menu.ns_menu().cast(), Ordering::Relaxed);
    _ = PROXY.set(Mutex::new(proxy));
    // AppKit asks the application's delegate for the Dock's menu and what a
    // click on the icon does. winit's delegate answers neither and winit
    // offers no way to answer them, so the methods are added to its class.
    // SAFETY: AppKit's application and its delegate live as long as the app.
    let delegate: *mut AnyObject = unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        msg_send![app, delegate]
    };
    // SAFETY: a delegate AppKit hands out is a live object, or none.
    let Some(delegate) = (unsafe { delegate.as_ref() }) else {
        tracing::warn!("the Dock is not answered: the app has no delegate");
        return;
    };
    let class: *const AnyClass = delegate.class();
    let bool = Bool::ENCODING.to_string();
    let methods = [
        (
            sel!(applicationDockMenu:),
            // SAFETY: the function takes and returns what the selector does.
            unsafe { std::mem::transmute::<DockMenu, Imp>(dock_menu) },
            "@@:@".to_owned(),
        ),
        (
            sel!(applicationShouldHandleReopen:hasVisibleWindows:),
            // SAFETY: the function takes and returns what the selector does.
            unsafe { std::mem::transmute::<Reopen, Imp>(reopen) },
            format!("{bool}@:@{bool}"),
        ),
    ];
    for (name, imp, types) in methods {
        let Ok(types) = CString::new(types) else {
            continue;
        };
        // SAFETY: `imp` takes and returns what `types` says, which is what
        // AppKit calls `name` with.
        let added =
            unsafe { objc2::ffi::class_addMethod(class.cast_mut(), name, imp, types.as_ptr()) };
        if !added.as_bool() {
            tracing::warn!(
                ?name,
                "the Dock is not answered: the delegate already answers"
            );
        }
    }
}

/// The menu bar with the keys of `keymap`, made the app's, and its
/// Sessions menu.
#[cfg(target_os = "macos")]
fn bar(keymap: &Keymap) -> muda::Result<(muda::Menu, muda::Submenu)> {
    let (menu, sessions, window, help) = platform::build(keymap)?;
    menu.init_for_nsapp();
    // Only once the menu bar is the app's: before, muda finds no menu of its
    // own to hand AppKit, and the window list and the Help search field are
    // left out.
    window.set_as_windows_menu_for_nsapp();
    help.set_as_help_menu_for_nsapp();
    Ok((menu, sessions))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod platform {
    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    use winit::keyboard::ModifiersState;

    use super::{Line, Listed, id};
    use crate::keymap::{Chord, ChordKey, Keymap};
    use crate::keys::Command;

    /// An item for `command`, with the key `keymap` binds it to where the
    /// menu shows keys.
    fn item(command: Command, label: &str, keymap: Option<&Keymap>) -> MenuItem {
        let key = keymap
            .and_then(|keymap| keymap.chord(command))
            .and_then(accelerator);
        MenuItem::with_id(id(command), label, true, key)
    }

    /// The accelerator a menu shows and answers for `chord`, where a key of
    /// the US layout types its character.
    fn accelerator(chord: Chord) -> Option<Accelerator> {
        const SHIFTED: &[(char, char)] = &[
            ('<', ','),
            ('>', '.'),
            ('?', '/'),
            (':', ';'),
            ('"', '\''),
            ('{', '['),
            ('}', ']'),
            ('|', '\\'),
            ('_', '-'),
            ('+', '='),
            ('~', '`'),
            ('!', '1'),
            ('@', '2'),
            ('#', '3'),
            ('$', '4'),
            ('%', '5'),
            ('^', '6'),
            ('&', '7'),
            ('*', '8'),
            ('(', '9'),
            (')', '0'),
        ];
        let mut mods = Modifiers::empty();
        for (held, modifier) in [
            (ModifiersState::SUPER, Modifiers::META),
            (ModifiersState::CONTROL, Modifiers::CONTROL),
            (ModifiersState::ALT, Modifiers::ALT),
            (ModifiersState::SHIFT, Modifiers::SHIFT),
        ] {
            if chord.mods.contains(held) {
                mods |= modifier;
            }
        }
        let name = match chord.key {
            ChordKey::Named(named) => format!("{named:?}"),
            ChordKey::Char(' ') => "Space".to_owned(),
            ChordKey::Char(c) => {
                let c = match SHIFTED.iter().find(|(shifted, _)| *shifted == c) {
                    Some((_, base)) => {
                        mods |= Modifiers::SHIFT;
                        *base
                    }
                    None => c,
                };
                match c {
                    'a'..='z' => format!("Key{}", c.to_ascii_uppercase()),
                    '0'..='9' => format!("Digit{c}"),
                    ',' => "Comma".to_owned(),
                    '.' => "Period".to_owned(),
                    '/' => "Slash".to_owned(),
                    ';' => "Semicolon".to_owned(),
                    '\'' => "Quote".to_owned(),
                    '[' => "BracketLeft".to_owned(),
                    ']' => "BracketRight".to_owned(),
                    '\\' => "Backslash".to_owned(),
                    '-' => "Minus".to_owned(),
                    '=' => "Equal".to_owned(),
                    '`' => "Backquote".to_owned(),
                    _ => return None,
                }
            }
        };
        let code: Code = name.parse().ok()?;
        Some(Accelerator::new(mods, code))
    }

    /// A session's submenu: show it in the window, or end it.
    fn session(listed: &Listed) -> muda::Result<Submenu> {
        Submenu::with_items(
            &listed.title,
            true,
            &[
                &item(Command::ShowSession(listed.id), "Show in This Window", None),
                &item(Command::EndSessionOf(listed.id), "End Session", None),
            ],
        )
    }

    /// A menu of `lines`, as the platform draws one.
    pub fn menu(lines: &[Line]) -> muda::Result<Menu> {
        let menu = Menu::new();
        for line in lines {
            match line {
                Line::Item(command, label) => menu.append(&item(*command, label, None))?,
                Line::Separator => menu.append(&PredefinedMenuItem::separator())?,
                Line::Session(listed) => menu.append(&session(listed)?)?,
            }
        }
        Ok(menu)
    }

    /// Fill the menu bar's Sessions menu with `listed` anew.
    #[cfg(target_os = "macos")]
    pub fn list(menu: &Submenu, listed: &[Listed], keymap: &Keymap) -> muda::Result<()> {
        while menu.remove_at(0).is_some() {}
        menu.append(&item(Command::NextSession, "Next Session", Some(keymap)))?;
        menu.append(&item(
            Command::PreviousSession,
            "Previous Session",
            Some(keymap),
        ))?;
        menu.append(&PredefinedMenuItem::separator())?;
        for listed in listed {
            menu.append(&session(listed)?)?;
        }
        if !listed.is_empty() {
            menu.append(&PredefinedMenuItem::separator())?;
        }
        menu.append(&item(Command::EndSession, "End Session", Some(keymap)))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn build(keymap: &Keymap) -> muda::Result<(Menu, Submenu, Submenu, Submenu)> {
        use felis_protocol::messages::PromptJump;

        use crate::settings::Zoom;
        let app = Submenu::with_items(
            "Cattery",
            true,
            &[
                &item(Command::About, "About Cattery", Some(keymap)),
                &PredefinedMenuItem::separator(),
                &item(Command::Preferences, "Preferences…", Some(keymap)),
                &item(Command::Reload, "Reload Configuration", Some(keymap)),
                &item(Command::RestartDaemon, "Restart Daemon…", Some(keymap)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(Some("Hide Cattery")),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &item(Command::Quit, "Quit Cattery", Some(keymap)),
            ],
        )?;
        let shell = Submenu::with_items(
            "File",
            true,
            &[
                &item(Command::NewWindow, "New Window", Some(keymap)),
                &item(Command::NewWindowHere, "New Window Here", Some(keymap)),
                &item(Command::Palette, "Command Palette…", Some(keymap)),
                &PredefinedMenuItem::separator(),
                &item(Command::CloseWindow, "Close Window", Some(keymap)),
                &item(
                    Command::EndSession,
                    "Close Window and End Session",
                    Some(keymap),
                ),
                &item(Command::CloseAllWindows, "Close All Windows", Some(keymap)),
            ],
        )?;
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[
                &item(Command::Copy, "Copy", Some(keymap)),
                &item(Command::Paste, "Paste", Some(keymap)),
                &PredefinedMenuItem::separator(),
                &item(Command::Find, "Find…", Some(keymap)),
            ],
        )?;
        let view = Submenu::with_items(
            "View",
            true,
            &[
                &item(Command::Zoom(Zoom::In), "Bigger", Some(keymap)),
                &item(Command::Zoom(Zoom::Out), "Smaller", Some(keymap)),
                &item(Command::Zoom(Zoom::Reset), "Default Size", Some(keymap)),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::Prompt(PromptJump::Previous),
                    "Previous Prompt",
                    Some(keymap),
                ),
                &item(
                    Command::Prompt(PromptJump::Next),
                    "Next Prompt",
                    Some(keymap),
                ),
            ],
        )?;
        let sessions = Submenu::new("Sessions", true);
        list(&sessions, &[], keymap)?;
        let window = Submenu::with_items(
            "Window",
            true,
            &[
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::maximize(Some("Zoom")),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::bring_all_to_front(None),
            ],
        )?;
        let help = Submenu::with_items(
            "Help",
            true,
            &[
                &item(Command::Help, "Cattery Help", Some(keymap)),
                &item(Command::ReportIssue, "Report an Issue…", Some(keymap)),
            ],
        )?;
        let menu = Menu::with_items(&[&app, &shell, &edit, &view, &sessions, &window, &help])?;
        Ok((menu, sessions, window, help))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use felis_protocol::messages::PromptJump;

    #[test]
    fn every_command_is_told_by_its_menu_item() {
        for command in [
            Command::About,
            Command::Preferences,
            Command::NewWindow,
            Command::NewWindowHere,
            Command::CloseWindow,
            Command::Find,
            Command::Palette,
            Command::Copy,
            Command::Paste,
            Command::Quit,
            Command::Zoom(Zoom::In),
            Command::Zoom(Zoom::Out),
            Command::Zoom(Zoom::Reset),
            Command::Prompt(PromptJump::Previous),
            Command::Prompt(PromptJump::Next),
            Command::CloseAllWindows,
            Command::Reload,
            Command::RestartDaemon,
            Command::Help,
            Command::ReportIssue,
            Command::NextSession,
            Command::PreviousSession,
            Command::EndSession,
            Command::ShowSession(0xdead_beef_0123_4567_89ab_cdef_0011_2233),
            Command::EndSessionOf(u128::MAX),
        ] {
            assert_eq!(super::command(&id(command)), Some(command), "{command:?}");
        }
    }

    #[test]
    fn every_item_of_a_window_s_menus_tells_its_command() {
        for entry in WINDOW_MENU.iter().chain(APP_MENU).chain(DOCK_MENU) {
            if let Entry::Item(command, label) = entry {
                assert_eq!(super::command(&id(*command)), Some(*command), "{label}");
            }
        }
    }

    #[test]
    fn the_dock_offers_a_new_window() {
        assert_eq!(
            lines(DOCK_MENU, &[]),
            [Line::Item(Command::NewWindow, "New Window".to_owned())]
        );
    }

    #[test]
    fn no_menu_starts_or_ends_with_a_separator() {
        for menu in [WINDOW_MENU, APP_MENU, DOCK_MENU] {
            assert_ne!(menu.first(), Some(&Entry::Separator));
            assert_ne!(menu.last(), Some(&Entry::Separator));
        }
    }

    #[test]
    fn the_sessions_take_their_place_in_the_menu() {
        let listed = Listed {
            id: 7,
            title: "~/src".to_owned(),
        };
        let menu = [
            Entry::Item(Command::NextSession, "Next Session"),
            Entry::Separator,
            Entry::Sessions,
            Entry::Separator,
            Entry::Item(Command::EndSession, "End Session"),
        ];
        assert_eq!(
            lines(&menu, std::slice::from_ref(&listed)),
            [
                Line::Item(Command::NextSession, "Next Session".to_owned()),
                Line::Separator,
                Line::Session(listed),
                Line::Separator,
                Line::Item(Command::EndSession, "End Session".to_owned()),
            ]
        );
    }

    #[test]
    fn no_sessions_leave_no_doubled_separator() {
        let menu = [
            Entry::Item(Command::NextSession, "Next Session"),
            Entry::Separator,
            Entry::Sessions,
            Entry::Separator,
            Entry::Item(Command::EndSession, "End Session"),
        ];
        assert_eq!(
            lines(&menu, &[]),
            [
                Line::Item(Command::NextSession, "Next Session".to_owned()),
                Line::Separator,
                Line::Item(Command::EndSession, "End Session".to_owned()),
            ]
        );
    }

    #[test]
    fn an_item_of_the_platform_s_own_stands_for_no_command() {
        assert_eq!(command("minimize"), None);
    }
}
