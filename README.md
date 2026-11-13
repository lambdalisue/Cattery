<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/cattery-icon-dark.svg">
    <img src="assets/logo/cattery-icon-light.svg" alt="" width="128">
  </picture>
</p>

# Cattery

A GUI terminal app built on the felis terminal daemon

Cattery (a place that houses many cats) is a desktop app for
[felis](https://github.com/felis-terminal/felis) sessions. felis keeps each
session in a daemon and leaves windows to the window manager; Cattery is an
app that owns its windows: it opens a new one from its own menu, has an app
icon, and lets settings be changed in a Preferences window.

> [!NOTE]
> Cattery is at the very start of development: windows on sessions, a menu,
> themes, and a Preferences window, and not much more yet.

## Install

Not released yet. With Nix, build it from the repository:

```sh
nix profile add github:lambdalisue/cattery
```

On macOS the package holds `Applications/Cattery.app`, which Finder,
Launchpad and the Dock show as Cattery; the `cattery` on your `PATH` is the
same app.

## Usage

From the development shell:

```sh
cargo run
```

A window opens on your login shell. Closing it ends the session.

The Shell menu opens more: **New Window** (Command-N) in your home
directory, **New Window Here** (Command-Shift-N) where the shell in front
last reported being. On Linux and Windows the chords are Control-Shift-N
and Control-Alt-Shift-N.

Command-comma (Control-comma on Linux and Windows) opens the Preferences
window, where the theme, the font, the cursor and the shell are chosen.
The settings are kept in `cattery/` under your configuration directory:
`config.toml` in felis's format, and `appearance.toml` for the theme.

## Development

Enter the development shell with `nix develop` (or copy `.envrc.sample` to
`.envrc` and `direnv allow`), then:

```sh
just fmt   # format every source
just ci    # everything CI runs: format check, lint, tests
```

The design lives in [docs/concept.md](docs/concept.md), the decisions behind it
in [docs/adr/](docs/adr/), and the behaviour it promises in
[docs/spec/](docs/spec/).

## License

MIT. See [LICENSE](LICENSE).
