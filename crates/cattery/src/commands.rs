//! The commands the palette offers: what each is called, and the chord
//! that runs it on this platform.

use felis_protocol::messages::PromptJump;

use crate::keys::Command;
use crate::settings::Zoom;

/// The commands the palette lists, in the order it lists them.
pub const LISTED: &[Command] = &[
    Command::NewWindow,
    Command::NewWindowHere,
    Command::Find,
    Command::Prompt(PromptJump::Previous),
    Command::Prompt(PromptJump::Next),
    Command::Copy,
    Command::Paste,
    Command::Zoom(Zoom::In),
    Command::Zoom(Zoom::Out),
    Command::Zoom(Zoom::Reset),
    Command::NextSession,
    Command::PreviousSession,
    Command::EndSession,
    Command::CloseWindow,
    Command::CloseAllWindows,
    Command::Preferences,
    Command::Reload,
    Command::RestartDaemon,
    Command::About,
    Command::Help,
    Command::ReportIssue,
    Command::Quit,
];

/// What `command` is called, as the menu calls it.
pub const fn label(command: Command) -> &'static str {
    match command {
        Command::About => "About Cattery",
        Command::Preferences => "Preferences…",
        Command::NewWindow => "New Window",
        Command::NewWindowHere => "New Window Here",
        Command::CloseWindow => "Close Window",
        Command::CloseAllWindows => "Close All Windows",
        Command::Reload => "Reload Configuration",
        Command::RestartDaemon => "Restart Daemon…",
        Command::Help => "Cattery Help",
        Command::ReportIssue => "Report an Issue…",
        Command::Palette => "Command Palette…",
        Command::Find => "Find…",
        Command::Copy => "Copy",
        Command::Paste => "Paste",
        Command::Quit => "Quit Cattery",
        Command::Zoom(Zoom::In) => "Bigger",
        Command::Zoom(Zoom::Out) => "Smaller",
        Command::Zoom(Zoom::Reset) => "Default Size",
        Command::Prompt(PromptJump::Previous) => "Previous Prompt",
        Command::Prompt(PromptJump::Next) => "Next Prompt",
        Command::NextSession => "Next Session",
        Command::PreviousSession => "Previous Session",
        Command::EndSession | Command::EndSessionOf(_) => "End Session",
        Command::ShowSession(_) => "Show Session",
    }
}

/// The chord that runs `command` here, written as the platform writes
/// chords; `None` where it has none.
pub const fn chord(command: Command) -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some(match command {
            Command::Preferences => "⌘,",
            Command::Reload => "⇧⌘,",
            Command::NewWindow => "⌘N",
            Command::NewWindowHere => "⇧⌘N",
            Command::CloseWindow => "⌘W",
            Command::CloseAllWindows => "⌥⌘W",
            Command::Palette => "⌘P",
            Command::Find => "⌘F",
            Command::Copy => "⌘C",
            Command::Paste => "⌘V",
            Command::Quit => "⌘Q",
            Command::Zoom(Zoom::In) => "⌘=",
            Command::Zoom(Zoom::Out) => "⌘-",
            Command::Zoom(Zoom::Reset) => "⌘0",
            Command::Prompt(PromptJump::Previous) => "⌘↑",
            Command::Prompt(PromptJump::Next) => "⌘↓",
            Command::NextSession => "⇧⌘]",
            Command::PreviousSession => "⇧⌘[",
            Command::EndSession => "⇧⌘W",
            Command::About
            | Command::Help
            | Command::ReportIssue
            | Command::RestartDaemon
            | Command::ShowSession(_)
            | Command::EndSessionOf(_) => return None,
        })
    } else {
        Some(match command {
            Command::Preferences => "Ctrl+,",
            Command::NewWindow => "Ctrl+Shift+N",
            Command::NewWindowHere => "Ctrl+Alt+Shift+N",
            Command::CloseWindow => "Ctrl+Shift+W",
            Command::Palette => "Ctrl+Shift+P",
            Command::Find => "Ctrl+Shift+F",
            Command::Copy => "Ctrl+Shift+C",
            Command::Paste => "Ctrl+Shift+V",
            Command::Zoom(Zoom::In) => "Ctrl+Shift+=",
            Command::Zoom(Zoom::Out) => "Ctrl+Shift+-",
            Command::Zoom(Zoom::Reset) => "Ctrl+Shift+0",
            Command::Prompt(PromptJump::Previous) => "Ctrl+Shift+Z",
            Command::Prompt(PromptJump::Next) => "Ctrl+Shift+X",
            Command::NextSession => "Ctrl+Shift+PageDown",
            Command::PreviousSession => "Ctrl+Shift+PageUp",
            Command::EndSession => "Ctrl+Alt+Shift+W",
            Command::About
            | Command::Help
            | Command::ReportIssue
            | Command::Quit
            | Command::Reload
            | Command::RestartDaemon
            | Command::CloseAllWindows
            | Command::ShowSession(_)
            | Command::EndSessionOf(_) => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_does_not_list_itself() {
        assert!(!LISTED.contains(&Command::Palette));
    }

    #[test]
    fn every_listed_command_is_listed_once() {
        for (at, command) in LISTED.iter().enumerate() {
            assert!(!LISTED[at + 1..].contains(command), "{command:?}");
        }
    }

    #[test]
    fn a_hint_names_the_chord_keys_take() {
        // The same chords keys::command answers to.
        let new_window = if cfg!(target_os = "macos") {
            "⌘N"
        } else {
            "Ctrl+Shift+N"
        };
        assert_eq!(chord(Command::NewWindow), Some(new_window));
        assert_eq!(chord(Command::Help), None);
    }
}
