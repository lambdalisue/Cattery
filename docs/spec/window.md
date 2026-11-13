# Window

What a Cattery window is and what happens to the session it shows.

## Opening

Starting Cattery opens one window showing a new session, which runs the
user's login shell in the directory Cattery was started from, with
Cattery's environment.

The session lives in Cattery's own felis daemon. Cattery starts that daemon
when none is running, and it keeps running after the window closes. It is
apart from any felis daemon the user runs: a felis client does not see
Cattery's sessions, and Cattery does not see theirs.

Every window shows a session of its own. More windows open from the one in
front:

- **New Window** opens a window whose session starts in the home directory.
- **New Window Here** opens a window whose session starts in the directory
  the shell in the window in front last reported (`OSC 7`); without a
  report, it starts in the home directory.

On macOS, Command-N and Command-Shift-N open them. On Linux and Windows,
where Control-letter belongs to the terminal, Control-Shift-N and
Control-Alt-Shift-N do.

The menu offers the same, with About Cattery, Preferences and Close
Window: the menu bar on macOS, and each window's menu on Windows. Linux
has no menu; the chords work there as everywhere.

## While it is open

The window shows the session's screen as felis draws it, and every key
typed in the window reaches the session. Text typed through the platform's
input method is shown at the cursor while it is composed, with the input
method's candidates beside it, and reaches the session once committed.
Resizing the window resizes the session to the cells that fit.

Cattery's icon — in the Dock on macOS, and in the title bar and the taskbar
where the platform shows one there — is light while the light theme is in
force and dark while the dark theme is, and changes with it.

A window is called what the program in it last called it (`OSC 0` or
`OSC 2`), and "Cattery" until it does, behind the settings'
`window.title_prefix`.

## Closing

A window closes when the program in its session ends, and closing the
window ends the session. Command-W on macOS, and Control-Shift-W on Linux
and Windows, close the window in front. Cattery ends with its last window.
Quitting Cattery closes every window, ending every session.
