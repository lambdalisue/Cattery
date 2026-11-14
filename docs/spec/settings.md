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

## Themes

Cattery's colours come from a theme: one for a light appearance, one for a
dark, and the appearance that picks between them — the system's, which
Cattery follows as it changes, or always light, or always dark. Where the
system's appearance cannot be told, it is taken for dark.

They are kept in `appearance.toml`, in the directory of the settings file,
with how the window shows what is behind it:

```toml
appearance = "system"   # "system", "light" or "dark"
light_theme = "Dawnfox"
dark_theme = "Nightfox"
opacity = 1.0           # how much of the background is drawn, 0.0 to 1.0
blur = 0                # how far what shows through is blurred, in points
```

A key left out has the value above. A file that does not read as this
leaves the appearance in use, and at start the one above. An opacity or a
blur out of range is brought to the nearest it can be. The `window.opacity`
of the settings file is not read.

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
