# Settings

How Cattery is told how to look and what to run.

## The file

Cattery's settings are kept in `config.toml` in a `Cattery` directory under
the platform's configuration directory (`~/Library/Application Support` on
macOS, `%APPDATA%` on Windows), or `cattery` under `$XDG_CONFIG_HOME` or
`~/.config` on Linux. A `cattery` directory an earlier Cattery made on macOS
or Windows is renamed `Cattery` when Cattery starts.
`CATTERY_CONFIG` names another file instead. Without a file, every setting
has felis's default.

The file is written in [felis's configuration format][felis-config] and read
by felis's own reader, so what felis accepts there Cattery accepts, and what
felis warns about it warns about too. Settings in a `[client.cattery]`
section apply to Cattery only; a `[client.<name>]` section for another
client does not. The file is Cattery's own: felis's file is not read, and
Cattery's does not change felis's windows.

[felis-config]: https://github.com/felis-terminal/felis/blob/main/docs/reference/config.md

## Changing it

Saving the file, or the appearance file below, applies the change to every
open window, without a restart, except the shell a session runs, which applies
to the sessions started after the change.

A file felis's reader refuses changes nothing: the settings in use stay, and
the reason is logged. At start, such a file leaves every setting at its
default.

Programs that ask the terminal for its colours are told the colours the
settings and the theme give.

## The font

Cattery ships a fixed-width font, JetBrains Mono Nerd Font Mono, which
carries the icons shell prompts draw. A grid is drawn with it unless the
settings name another family, and a family the settings name is looked for
among the faces Cattery ships before the system's. What the faces in use
lack, Japanese among it, is drawn with the system's fonts.

On macOS, a character the font and the fallbacks the settings name lack is
looked for in Hiragino Sans before the system picks a face for it, so Han
characters are drawn with Japanese glyphs whatever the preferred language.

## Themes

Cattery's colours come from a theme: one for a light appearance, one for a
dark, and the appearance that picks between them — the system's, which
Cattery follows as it changes, or always light, or always dark. Where the
system's appearance cannot be told, it is taken for dark.

They are kept in `appearance.toml`, in the directory of the settings file,
with how the window shows what is behind it and how big a new one opens:

```toml
appearance = "system"   # "system", "light" or "dark"
light_theme = "Dawnfox"
dark_theme = "Nightfox"
opacity = 1.0           # how much of the background is drawn, 0.0 to 1.0
blur = 0                # how far what shows through is blurred, in points
columns = 120           # the columns a new window holds, 1 to 1000
rows = 35               # the rows a new window holds, 1 to 1000
```

A key left out has the value above, but for `columns` and `rows`: without
them a new window is as wide, or as tall, as the platform opens it. A file
that does not read as this leaves the appearance in use, and at start the
default. An opacity, a blur, or columns or rows out of range are brought to
the nearest they can be. The `window.opacity` of the settings file is not
read.

A new window holds the columns and rows at the text size the settings give.
A change to them applies to the windows opened after it; the windows open
keep their size.

The blur is drawn on macOS, by the radius it gives, while the window is
translucent. Elsewhere a blur above 0 asks the compositor to blur behind the
window, where it can.

A theme is named by its file in Ghostty's theme format, in the `themes`
directory beside the settings file, or by one Cattery comes with. A user's
theme of the same name as one of Cattery's takes its place. Only the
palette's sixteen colours, `background`, `foreground` and `cursor-color`
are read; the rest of Ghostty's keys colour what felis has no setting for.
A theme of no such name, or one that does not read, leaves felis's colours.

A colour the settings file sets is drawn in place of the theme's.

## Keys

The keys that run Cattery's commands are bound in `keys.toml`, in the
directory of the settings file, over the keys Cattery binds of itself. Each
line binds one key to one command, or takes the key's binding away:

```toml
"Mod+t" = "window.new"
"Ctrl+Alt+p" = "palette.open"
"Mod+p" = "none"
```

A key is written as modifiers and a key joined by `+`. The modifiers are
`Cmd` (Super off macOS), `Ctrl`, `Alt`, `Shift`, and `Mod`, which is `Cmd`
on macOS and `Ctrl+Shift` on Linux and Windows, where Control with a letter
is the terminal's. The key is a character, a named key — `Enter`, `Escape`,
`Tab`, `Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`,
`PageDown`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `F1` to
`F12` — or `Space` or `Plus`. A capital letter holds Shift. Any other
character is the one the layout types, Shift included: on a US layout
Shift with `]` is written `}`, and matches with or without Shift held
unless `Shift` is written.

The commands are `app.about`, `app.preferences`, `app.reload`,
`app.restart_daemon`, `app.help`, `app.report_issue`, `app.quit`,
`window.new`, `window.new_here`, `window.close`, `window.close_all`,
`session.next`, `session.previous`, `session.end`, `palette.open`,
`search.find`, `edit.copy`, `edit.paste`, `view.zoom_in`, `view.zoom_out`,
`view.zoom_reset`, `prompt.previous` and `prompt.next`; `none` takes the key
away. A key bound in the file runs its command in place of what Cattery
binds to it, and the menus and the palette show it.

A line that does not read — an unknown modifier, key or command — is left
out and logged, and the rest of the file is taken; a file that does not read
as TOML leaves Cattery's own keys. Saving the file applies it to every open
window. felis's `[keymap]` is not read.
