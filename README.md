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
> Cattery is at the very start of development. It opens one window on a
> session, and its settings can be changed in a Preferences window; there is
> no menu or icon yet.

## Install

Not released yet.

## Usage

From the development shell:

```sh
cargo run
```

A window opens on your login shell. Closing it ends the session.

Command-comma (Control-comma on Linux and Windows) opens the Preferences
window. The settings are kept in felis's configuration format, in
`cattery/config.toml` under your configuration directory.

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
