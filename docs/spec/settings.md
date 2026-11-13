# Settings

How Cattery is told how to look and what to run.

## The file

Cattery's settings are kept in `config.toml` in a `cattery` directory under
the platform's configuration directory (`$XDG_CONFIG_HOME` or `~/.config` on
Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows).
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

Saving the file applies the change to every open window, without a restart,
with these exceptions:

- The shell a session runs applies to the sessions started after the change.
- A window is opaque or translucent from when it opens: a change of opacity
  across fully opaque applies to windows opened after it.

A file felis's reader refuses changes nothing: the settings in use stay, and
the reason is logged. At start, such a file leaves every setting at its
default.

Programs that ask the terminal for its colours are told the colours the
settings give.
