# Bind Cattery's keys in a file of its own

Status: Accepted

## Context

Cattery's commands — new windows, the palette, Preferences, the sessions —
each answer to a fixed key, and users want to choose their own. Cattery
keeps what felis's settings define in felis's format, by
`20261114-keep-cattery-s-settings-in-felis-s-format-in-a-file-of-its-own.md`.
felis's settings hold a `[keymap]`, but it binds keys only to felis's own
closed set of actions, which has no place for most of Cattery's commands.

## Decision

Cattery's keys are bound in a file of Cattery's own, naming Cattery's
commands, over the keys Cattery binds of itself. felis's `[keymap]` is not
read.

## Alternatives considered

- Bind keys in felis's `[keymap]`. It stays within felis's format, but
  Cattery's own commands cannot be named there until felis takes them.
- Read felis's `[keymap]` for what it can name, and a file of Cattery's for
  the rest. One key's binding could then be in either file.

## Consequences

- Every command Cattery offers can be bound, and the menus and the palette
  show the keys bound.
- The keys of felis's `[keymap]` change nothing in Cattery.
- If felis lets a client name actions of its own in `[keymap]`, the keys may
  move into felis's settings with a new record.
