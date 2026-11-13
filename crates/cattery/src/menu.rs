//! Cattery's menu: the menu bar on macOS, and each window's on Windows.
//!
//! Linux has none: muda draws menus into GTK windows, and winit's windows
//! there are not GTK's. The chords in [`crate::keys::command`] do the same
//! on every platform.

use winit::event_loop::EventLoopProxy;
use winit::window::Window;

use felis_protocol::messages::PromptJump;

use crate::app::Event;
use crate::keys::Command;
use crate::settings::Zoom;

const COMMANDS: [(Command, &str); 19] = [
    (Command::CloseAllWindows, "close-all-windows"),
    (Command::Reload, "reload"),
    (Command::Help, "help"),
    (Command::ReportIssue, "report-issue"),
    (Command::Prompt(PromptJump::Previous), "previous-prompt"),
    (Command::Prompt(PromptJump::Next), "next-prompt"),
    (Command::Find, "find"),
    (Command::Sessions, "sessions"),
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

/// The app's menu, while it is shown.
pub struct Menu {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    _menu: muda::Menu,
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
            #[cfg(target_os = "windows")]
            let menu = platform::build()?;
            Ok(Self { _menu: menu })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = proxy;
            Ok(Self {})
        }
    }

    /// Give `window` the menu, where windows carry their own.
    pub fn attach(&self, window: &Window) {
        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                // SAFETY: winit's window is alive while it is being handed.
                if let Err(err) = unsafe { self._menu.init_for_hwnd(handle.hwnd.get()) } {
                    tracing::warn!(?err, "the window's menu was not shown");
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window;
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod platform {
    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    use super::id;
    use felis_protocol::messages::PromptJump;

    use crate::keys::Command;
    use crate::settings::Zoom;

    fn item(command: Command, label: &str, chord: Option<(Modifiers, Code)>) -> MenuItem {
        let chord = chord.map(|(mods, code)| Accelerator::new(mods, code));
        MenuItem::with_id(id(command), label, true, chord)
    }

    #[cfg(target_os = "macos")]
    pub fn build() -> muda::Result<(Menu, Submenu, Submenu)> {
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
                    Command::Sessions,
                    "Sessions…",
                    Some((cmd | Modifiers::SHIFT, Code::KeyO)),
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

    #[cfg(target_os = "windows")]
    pub fn build() -> muda::Result<Menu> {
        let ctrl_shift = Modifiers::CONTROL | Modifiers::SHIFT;
        let file = Submenu::with_items(
            "&File",
            true,
            &[
                &item(
                    Command::NewWindow,
                    "&New Window",
                    Some((ctrl_shift, Code::KeyN)),
                ),
                &item(
                    Command::NewWindowHere,
                    "New Window &Here",
                    Some((ctrl_shift | Modifiers::ALT, Code::KeyN)),
                ),
                &item(
                    Command::Sessions,
                    "&Sessions…",
                    Some((ctrl_shift, Code::KeyO)),
                ),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::Preferences,
                    "&Preferences…",
                    Some((Modifiers::CONTROL, Code::Comma)),
                ),
                &item(Command::Reload, "&Reload Configuration", None),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::CloseWindow,
                    "&Close Window",
                    Some((ctrl_shift, Code::KeyW)),
                ),
                &item(Command::CloseAllWindows, "Close &All Windows", None),
                &item(Command::Quit, "E&xit", None),
            ],
        )?;
        let edit = Submenu::with_items(
            "&Edit",
            true,
            &[
                &item(Command::Copy, "&Copy", Some((ctrl_shift, Code::KeyC))),
                &item(Command::Paste, "&Paste", Some((ctrl_shift, Code::KeyV))),
                &PredefinedMenuItem::separator(),
                &item(Command::Find, "&Find…", Some((ctrl_shift, Code::KeyF))),
            ],
        )?;
        let view = Submenu::with_items(
            "&View",
            true,
            &[
                &item(
                    Command::Zoom(Zoom::In),
                    "&Bigger",
                    Some((ctrl_shift, Code::Equal)),
                ),
                &item(
                    Command::Zoom(Zoom::Out),
                    "&Smaller",
                    Some((ctrl_shift, Code::Minus)),
                ),
                &item(
                    Command::Zoom(Zoom::Reset),
                    "&Default Size",
                    Some((ctrl_shift, Code::Digit0)),
                ),
                &PredefinedMenuItem::separator(),
                &item(
                    Command::Prompt(PromptJump::Previous),
                    "&Previous Prompt",
                    Some((ctrl_shift, Code::KeyZ)),
                ),
                &item(
                    Command::Prompt(PromptJump::Next),
                    "&Next Prompt",
                    Some((ctrl_shift, Code::KeyX)),
                ),
            ],
        )?;
        let help = Submenu::with_items(
            "&Help",
            true,
            &[
                &item(Command::Help, "Cattery &Help", None),
                &item(Command::ReportIssue, "&Report an Issue…", None),
                &PredefinedMenuItem::separator(),
                &item(Command::About, "&About Cattery", None),
            ],
        )?;
        Menu::with_items(&[&file, &edit, &view, &help])
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
            Command::Sessions,
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
            Command::Help,
            Command::ReportIssue,
        ] {
            assert_eq!(super::command(id(command)), Some(command));
        }
    }

    #[test]
    fn an_item_of_the_platform_s_own_stands_for_no_command() {
        assert_eq!(command("minimize"), None);
    }
}
