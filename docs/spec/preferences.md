# Preferences

The window where Cattery's settings are changed.

## Opening it

Command-comma on macOS, and Control-comma on Linux and Windows, in a
session's window opens the Preferences window, or brings it to the front
when it is open. The chord does not reach the session. The window can be
resized and minimized, but not maximized.

## What it changes

- The font: its family, from the installed families felis can draw a grid
  with, and its size.
- The theme: the appearance, the light and the dark theme, the window's
  opacity, and on macOS how much what shows through a translucent window is
  blurred. Colours the settings file sets over the theme are listed, and can
  be given back to the theme.
- The columns and rows a new window holds, or the platform's width and
  height for it.
- The cursor: its colour, and whether it blinks always, never, or as the
  program asks.
- The command a new session runs, typed as a shell line; left empty, a
  session runs the login shell.

Each setting can be put back to felis's default, which takes it out of the
file.

## Saving

Every change is written as it is made — to the settings file, or for the
theme to the appearance file — and applies the way an edit to the file
does (see `settings.md`). The themes folder can be opened from the window. What else the file holds —
comments, other settings, their order — stays as it was. A change felis's
reader would refuse is not written, and the window says why. A settings file
that is a link to one kept elsewhere is not written through; it is changed
where it is kept.

Changes made to the file while the window is open are shown in it.

## About

The window also has a page about Cattery: its version, the felis revision
it is built with, the files its settings are kept in — each to be opened or
shown in the file manager — and where its source, its issues and felis
are.
**About Cattery** in the menu opens the window on it.
