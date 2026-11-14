//! Cattery's menus: the menu bar on macOS, a window's menu where it is
//! right-clicked, and on Linux and Windows the app's menu behind the menu
//! button of the title bar Cattery draws. The chords in
//! [`crate::keys::command`] do the same on every platform.

use winit::dpi::PhysicalPosition;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

use felis_protocol::messages::PromptJump;

use crate::app::Event;
use crate::keys::Command;
use crate::settings::Zoom;

const COMMANDS: [(Command, &str); 20] = [
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
];

/// The id a menu item for `command` carries.
pub fn id(command: Command) -> &'static str {
    COMMANDS
        .iter()
        .find(|(each, _)| *each == command)
        .map_or("", |(_, id)| id)
}

/// The command the menu item `id` stands for.
pub fn command(id: &str) -> Option<Command> {
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

/// What the menu button of the title bar Cattery draws offers: all the menu
/// bar does on macOS.
pub const APP_MENU: &[Entry] = &[
    Entry::Item(Command::NewWindow, "New Window"),
    Entry::Item(Command::NewWindowHere, "New Window Here"),
    Entry::Item(Command::Palette, "Command Palette…"),
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
    Entry::Item(Command::CloseAllWindows, "Close All Windows"),
    Entry::Item(Command::Quit, "Quit"),
];

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
    /// What right-clicking a window offers, as the platform draws a menu.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    window_menu: muda::Menu,
    /// What the title bar's menu button offers.
    #[cfg(target_os = "windows")]
    app_menu: muda::Menu,
}

impl Menu {
    /// Show the menu, sending what is chosen in it to `proxy`.
    pub fn install(proxy: EventLoopProxy<Event>) -> anyhow::Result<Self> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                if let Some(command) = command(event.id().as_ref()) {
                    _ = proxy.send_event(Event::Command(command));
                }
            }));
            #[cfg(target_os = "macos")]
            let menu = {
                let (menu, window, help) = platform::build()?;
                menu.init_for_nsapp();
                // Only once the menu bar is the app's: before, muda finds no
                // menu of its own to hand AppKit, and the window list and the
                // Help search field are left out.
                window.set_as_windows_menu_for_nsapp();
                help.set_as_help_menu_for_nsapp();
                menu
            };
            Ok(Self {
                #[cfg(target_os = "macos")]
                _menu: menu,
                window_menu: platform::entries(WINDOW_MENU)?,
                #[cfg(target_os = "windows")]
                app_menu: platform::entries(APP_MENU)?,
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = proxy;
            Ok(Self {})
        }
    }

    /// Show the menu `which` of `window`, at `at` in the window or else at
    /// the pointer, until an item is chosen or the menu is dismissed.
    pub fn pop_up(&self, window: &Window, which: Which, at: Option<PhysicalPosition<f64>>) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            use muda::ContextMenu as _;
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let menu = match which {
                Which::Window => &self.window_menu,
                // Only Windows has a title bar of Cattery's, with the button.
                #[cfg(target_os = "windows")]
                Which::App => &self.app_menu,
                #[cfg(target_os = "macos")]
                Which::App => return,
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
        let _ = (window, which, at);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod platform {
    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{Menu, MenuItem, PredefinedMenuItem};

    use super::{Entry, id};
    use crate::keys::Command;

    fn item(command: Command, label: &str, chord: Option<(Modifiers, Code)>) -> MenuItem {
        let chord = chord.map(|(mods, code)| Accelerator::new(mods, code));
        MenuItem::with_id(id(command), label, true, chord)
    }

    /// A menu of `entries`, as the platform draws one.
    pub fn entries(entries: &[Entry]) -> muda::Result<Menu> {
        let menu = Menu::new();
        for entry in entries {
            match *entry {
                Entry::Item(command, label) => menu.append(&item(command, label, None))?,
                Entry::Separator => menu.append(&PredefinedMenuItem::separator())?,
            }
        }
        Ok(menu)
    }

    #[cfg(target_os = "macos")]
    pub fn build() -> muda::Result<(Menu, muda::Submenu, muda::Submenu)> {
        use felis_protocol::messages::PromptJump;
        use muda::Submenu;

        use crate::settings::Zoom;

        let cmd = Modifiers::META;
        let app = Submenu::with_items(
            "Cattery",
            true,
            &[
                &item(Command::About, "About Cattery", None),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::Preferences,
                    "Preferences…",
                    Some((cmd, Code::Comma)),
                ),
                &item(
                    Command::Reload,
                    "Reload Configuration",
                    Some((cmd | Modifiers::SHIFT, Code::Comma)),
                ),
                &item(Command::RestartDaemon, "Restart Daemon…", None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(Some("Hide Cattery")),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &item(Command::Quit, "Quit Cattery", Some((cmd, Code::KeyQ))),
            ],
        )?;
        let shell = Submenu::with_items(
            "File",
            true,
            &[
                &item(Command::NewWindow, "New Window", Some((cmd, Code::KeyN))),
                &item(
                    Command::NewWindowHere,
                    "New Window Here",
                    Some((cmd | Modifiers::SHIFT, Code::KeyN)),
                ),
                &item(
                    Command::Palette,
                    "Command Palette…",
                    Some((cmd, Code::KeyP)),
                ),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::CloseWindow,
                    "Close Window",
                    Some((cmd, Code::KeyW)),
                ),
                &item(
                    Command::CloseAllWindows,
                    "Close All Windows",
                    Some((cmd | Modifiers::ALT, Code::KeyW)),
                ),
            ],
        )?;
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[
                &item(Command::Copy, "Copy", Some((cmd, Code::KeyC))),
                &item(Command::Paste, "Paste", Some((cmd, Code::KeyV))),
                &PredefinedMenuItem::separator(),
                &item(Command::Find, "Find…", Some((cmd, Code::KeyF))),
            ],
        )?;
        let view = Submenu::with_items(
            "View",
            true,
            &[
                &item(Command::Zoom(Zoom::In), "Bigger", Some((cmd, Code::Equal))),
                &item(
                    Command::Zoom(Zoom::Out),
                    "Smaller",
                    Some((cmd, Code::Minus)),
                ),
                &item(
                    Command::Zoom(Zoom::Reset),
                    "Default Size",
                    Some((cmd, Code::Digit0)),
                ),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::Prompt(PromptJump::Previous),
                    "Previous Prompt",
                    Some((cmd, Code::ArrowUp)),
                ),
                &item(
                    Command::Prompt(PromptJump::Next),
                    "Next Prompt",
                    Some((cmd, Code::ArrowDown)),
                ),
            ],
        )?;
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
                &item(Command::Help, "Cattery Help", None),
                &item(Command::ReportIssue, "Report an Issue…", None),
            ],
        )?;
        let menu = Menu::with_items(&[&app, &shell, &edit, &view, &window, &help])?;
        Ok((menu, window, help))
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
        ] {
            assert_eq!(super::command(id(command)), Some(command));
        }
    }

    #[test]
    fn every_item_of_a_window_s_menus_tells_its_command() {
        for entry in WINDOW_MENU.iter().chain(APP_MENU) {
            if let Entry::Item(command, label) = entry {
                assert_eq!(super::command(id(*command)), Some(*command), "{label}");
            }
        }
    }

    #[test]
    fn no_menu_starts_or_ends_with_a_separator() {
        for menu in [WINDOW_MENU, APP_MENU] {
            assert_ne!(menu.first(), Some(&Entry::Separator));
            assert_ne!(menu.last(), Some(&Entry::Separator));
        }
    }

    #[test]
    fn an_item_of_the_platform_s_own_stands_for_no_command() {
        assert_eq!(command("minimize"), None);
    }
}
