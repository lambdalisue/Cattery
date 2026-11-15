# Keep a new window's size in cells with the opacity and blur

Status: Accepted

## Context

A new window opened at the platform's default size, so the cells it held
depended on the font and on Cattery's margins. felis's settings have no key
for a window's size, and felis's reader reports a key of Cattery's own in
its format as unknown, which is why
`20261114-keep-the-window-s-opacity-and-blur-with-the-themes.md` put the
window's opacity and blur in Cattery's own file.

## Decision

The size a new window opens at is kept as columns and rows of cells in
Cattery's own file, with the opacity and the blur.

## Alternatives considered

- Keep the size in felis's settings under a key of Cattery's own. felis's
  reader reports it as unknown every time the file is read.
- Keep the size in pixels or points. The cells it holds would still change
  with the font and the margins, which is what a size is asked for.

## Consequences

- Every Cattery window setting felis has no key for is in Cattery's own file.
- A size in cells gives a window of another size as the font changes.
- If felis takes a size for its windows, the size may move into felis's
  settings with a new record.
