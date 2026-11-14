//! The commands the palette offers, and what each is called.

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
}
