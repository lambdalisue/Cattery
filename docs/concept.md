# Concept

This document describes what cattery is for and the ideas every design
decision rests on. It is rewritten in place as the concept evolves; decisions
built on it are recorded as ADRs.

## What it is

cattery is a GUI terminal app built on [felis](https://github.com/felis-terminal/felis).
felis splits a terminal into a daemon that holds the sessions — their
pseudo-terminals, emulator state, scrollback and images — and clients that
show them. cattery is such a client: a desktop app whose windows show
sessions kept in a felis daemon.

The name is a cattery: a place that houses many cats.

## Who it is for

People who like felis as a terminal but want the app around it to be an app:
one that opens a new window from its own menu, has an icon in the Dock or the
taskbar, and lets its settings be changed in a Preferences window rather than
only in a file.

## Principles

- **cattery owns its windows.** felis leaves tabs, splits and window layout to
  the window manager, and keeps its GUI to one window per session. cattery
  deliberately takes the other side: how sessions are arranged in windows is
  cattery's job. It starts with one window per session, and tabs and widgets
  are its to add, not a window manager's. This difference is the reason
  cattery exists.
- **A terminal behaves the way felis makes it behave.** cattery has no
  emulator, pseudo-terminal or key encoder of its own. A session looks and
  responds in cattery the way it does in felis's own window.
- **Settings are changed where they are seen.** Everything a user is expected
  to adjust can be adjusted from the Preferences window, and takes effect
  without a restart.
- **Light before rich.** A window opens fast and stays small. A feature that
  makes every window heavier has to earn it.
- **Self-contained.** cattery brings the felis it is built with and runs it
  itself; installing felis is not a prerequisite.
- **macOS, Linux and Windows alike.** A feature lands on every platform, or
  says why one is left out.

## What it does not do

- Emulate a terminal. Terminal behaviour that cattery wants and felis lacks is
  asked of felis.
- Share sessions with a felis daemon the user runs separately. cattery's
  sessions live in a daemon of its own.
- Keep sessions across a reboot or after its daemon exits; that is felis's
  boundary, and cattery inherits it.
