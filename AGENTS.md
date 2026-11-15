# Cattery

A GUI terminal app built on the felis terminal daemon

## How it is built

- **Test first.** Red → green → refactor. Write the failing test before the
  code that makes it pass.
- **Each layer holds one thing.** Put a fact in the one layer that owns it,
  and nowhere else.

  | Layer | Holds |
  | --- | --- |
  | `docs/concept.md` | what this is and for whom |
  | `docs/adr/` | one decision and why it was made |
  | `docs/spec/` | the behaviour users and other components can rely on |
  | Tests | the details that pin the behaviour down: rules, boundaries, tables |
  | Code | how it is done |
  | Commits | why a change was made |
  | Comments | why it is not done another way |

  Before writing, fixing or reviewing a record, read `docs/adr/README.md`;
  before writing, fixing or reviewing a spec, read `docs/spec/README.md`.
  Follow their writing order.
- **Dependencies are current.** Before adding or bumping one, look up its
  latest release in the registry. Do not copy a version from memory or from
  another repository.
- **felis is pinned to one git revision.** Every `felis-*` dependency in
  `Cargo.toml` names the same `rev`; move them together. felis is not on
  crates.io and promises no API, so a new revision may need Cattery changed.
  What Cattery needs and felis lacks is asked of felis upstream, with a
  stopgap here until it lands.

## Quality gate

Run `just ci` before reporting a change as done. CI runs the same recipe on
macOS, Linux and Windows.

## Layout

| Path | What lives there |
| --- | --- |
| `crates/cattery/` | the `cattery` app binary |
| `crates/cattery/themes/` | the themes Cattery ships, with their licences |
| `crates/cattery/fonts/` | the font Cattery ships, with the licences of it and of the icons in it |
| `assets/logo/` | the icon, the PNGs `scripts/icon.sh` renders from it, `cattery.icon` (the icon as Icon Composer keeps it, light and dark, for macOS 26), and the logos that set the name beside it in Quicksand Bold, outlined |
| `docs/` | concept, ADRs, specs, references, and the README's screenshots in `docs/images/` |
| `scripts/` | helpers the release and `just ci` run |

## Language

- README, docs and commit messages: English
- Code comments, log and error messages: English
- The app is written Cattery wherever it is named in prose or shown to
  users. The executable, the crate, paths and commit scopes stay `cattery`.

## Commits

Scoped Commits:

```
<scope>: <description>

<why this change is needed>
```

- The scope names where the change is, from the table below. Several scopes
  are joined with `, `; a change across the whole tree uses `treewide`.
- The description starts in lower case, has no trailing period, and stays
  within 72 characters.
- The body explains why. The code shows how; the tests show what.
- A release commit is `release: vX.Y.Z`.
- A breaking change says so in the body and carries a `BREAKING CHANGE:`
  trailer.

| Scope | Covers |
| --- | --- |
| `cattery` | `crates/cattery/`, `assets/` |
| `docs` | `README.md`, `AGENTS.md`, `docs/concept.md`, `docs/references/` |
| `adr` | `docs/adr/` |
| `spec` | `docs/spec/` |
| `ci` | `.github/` |
| `nix` | `flake.nix`, `flake.lock` |
| `just` | `justfile`, `scripts/` |
| `deps` | dependency updates, including the felis pin |

Add a row when a new area appears.
