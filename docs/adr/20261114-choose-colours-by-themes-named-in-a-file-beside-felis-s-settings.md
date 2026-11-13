# Choose colours by themes named in a file beside felis's settings

Status: Accepted

## Context

A terminal's colours are usually picked as a theme, one for a light
appearance and one for a dark, following the system's. felis's settings
hold one set of colours and have no theme, no appearance and no way to
follow the system's. Cattery's settings are kept in felis's format, by
`20261114-keep-cattery-s-settings-in-felis-s-format-in-a-file-of-its-own.md`,
which leaves what that format lacks to be asked of felis.

## Decision

Cattery chooses its colours by theme: a light theme and a dark one, named in
a file of Cattery's own beside its felis settings, with the appearance that
picks between them. Themes are in Ghostty's theme format; Cattery ships some,
and a user's own are files in a folder of Cattery's.

## Alternatives considered

- Write a chosen theme's colours into the felis settings. It stays within
  felis's format, but nothing there follows the system's appearance, and
  the theme a set of colours came from is lost once written.
- Wait for felis to take themes. Cattery would have no themes until then,
  and felis keeps theming to whoever draws its windows.

## Consequences

- Cattery's settings are in two files: felis's format for what felis
  defines, and Cattery's own for the themes.
- A colour set in the felis settings stands over the theme's, so a theme
  can be adjusted there.
- The themes Cattery ships are under their authors' licences, which ship
  with them.
- If felis takes themes, the file of Cattery's own may move into felis's
  settings with a new record.
