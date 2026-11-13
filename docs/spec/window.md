# Window

What a Cattery window is and what happens to the session it shows.

## Opening

Starting Cattery opens one window showing a new session, which runs the
user's login shell in the directory Cattery was started from, with
Cattery's environment.

A session's `TERM` is felis's `xterm-felis`, and its programs find that
terminfo entry wherever Cattery runs from: shipped beside Cattery in a
package, or compiled with the system's `tic` into Cattery's cache.

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
has no menu; the chords work there as everywhere. The menu also closes
every window at once (Close All Windows, leaving their sessions running),
reads the settings again (Reload Configuration, for an edit the watch on
the files missed), and opens Cattery's documentation and its issue tracker
from Help. On macOS the Window menu lists the open windows.

## While it is open

The window shows the session's screen as felis draws it, and every key
typed in the window reaches the session. Text typed through the platform's
input method is shown at the cursor while it is composed, with the input
method's candidates beside it, and reaches the session once committed.
Resizing the window resizes the session to the cells that fit.

Cattery's icon — in the Dock on macOS, and in the title bar and the taskbar
where the platform shows one there — is light while the light theme is in
force and dark while the dark theme is, and changes with it.

Find (Command-F on macOS, Control-Shift-F on Linux and Windows, or the Edit
menu) searches the session's history: the query is typed into a bar at the
bottom and sent with Return, and the matches are marked, the newest shown
first; Return or `n` shows the next older, `N` the next newer, and Escape —
or any other key, which then reaches the program — closes the bar.

On macOS, Option types the characters it composes (Option-E types é), as it
does in other apps; with the settings' `window.macos_option_as_alt`, it acts
as Alt (Meta) instead. A program ringing the bell in a window out of sight
calls for the user as the platform lets an app: the Dock icon bounces, or
the taskbar entry flashes.

A notification a program posts (OSC 9, 99 or 777), in any session Cattery's
daemon keeps, is shown as the platform shows an app's notifications, unless
its session is the one in the window in front. On macOS this takes Cattery
installed as an app, and the user's leave, which macOS asks for the first
time.

Images programs draw with the kitty graphics protocol are shown where they
place them, as felis draws them.

A shell that marks its prompts (OSC 133) can be stepped through them:
Previous Prompt and Next Prompt in the View menu, Command-Up and
Command-Down on macOS, Control-Shift-Z and Control-Shift-X on Linux and
Windows, bring the prompt before or after into view. zsh started in a window
is set up to mark its prompts and report its directory, after reading the
user's own startup files as it always does; fish marks them itself.

A link opens in the platform's default app when clicked with Command held
on macOS, or Control on Linux and Windows; the pointer turns to a hand over
one while the key is held. A link is one a program marked as such (OSC 8),
or else a web or file URL written out as plain text. Only web, mail and file
links open.

The mouse goes to a program that asks for it — clicks, drags, motion and
the wheel — unless Shift is held. Otherwise the wheel scrolls back through
the session's history, as Shift with Page Up and Page Down does by half a
screen, and on the alternate screen it sends a pager the arrow keys (the
page keys with Shift). Typing returns the view to the live screen.

Dragging selects text — a word with two clicks, a line with three, a
rectangle with Alt held — and Copy puts it on the clipboard (Command-C on
macOS, Control-Shift-C on Linux and Windows, or the Edit menu). Paste
(Command-V, Control-Shift-V) sends the clipboard's text, as does dropping a
file its path; text that would run commands in a program not taking it as
a paste — a line break without bracketed paste — waits for `y` first. A
program's write to the clipboard (`OSC 52`) reaches the system's only where
the settings' `clipboard.osc_52` is `system`.

A window's text grows and shrinks a pixel at a time, within the sizes
felis draws, and returns to the size the settings give: Command with `=`,
`-` and `0` on macOS, Control-Shift with them on Linux and Windows, or the
View menu. The size is the window's own, until the settings' size changes.

A window is called after the directory its shell last reported (`OSC 7`),
with the home directory as `~`, and what the program in it last called it
(`OSC 0` or `OSC 2`) — the directory once where that already names it —
and "Cattery" until either is told, behind the settings'
`window.title_prefix`.

## Closing

A window closes when the program in its session ends. Closing a window
leaves its session running in the daemon, out of sight, to be opened again
from the session palette; so does quitting Cattery, for every window.
Command-W on macOS, and Control-Shift-W on Linux and Windows, close the
window in front. Cattery ends with its last window.

## Sessions

The session palette (Command-Shift-O on macOS, Control-Shift-O on Linux and
Windows, or Sessions… in the File menu) opens over the window in front and
lists the sessions Cattery's daemon keeps, newest first, after an entry for
a new window: what each is called, where it is, and whether it is open in a
window, in the background and for how long, or ended. Typing narrows the
list to the sessions whose name or place holds the letters typed, in order.
The arrow keys choose; Return opens the session chosen in a new window, or
brings forward the window showing it; Command-Backspace (Control-Shift-
Backspace on Linux and Windows) ends it, and the program in it. Escape, or
turning to another window, closes the palette. The list follows sessions as
they start and end.
