# Keep the window's opacity and blur with the themes

Status: Accepted

## Context

Cattery keeps what felis's settings define in felis's format, by
`20261114-keep-cattery-s-settings-in-felis-s-format-in-a-file-of-its-own.md`,
and its themes in a file of its own, by
`20261114-choose-colours-by-themes-named-in-a-file-beside-felis-s-settings.md`.
felis's settings hold a window's opacity but no strength for the blur behind
it, so one window's look would be split between the two files, and a key of
Cattery's own in felis's format is reported as unknown by felis's reader.

## Decision

The window's opacity and the blur of what shows through it are kept in
Cattery's own file, with the themes. The opacity felis's settings hold is not
read.

## Alternatives considered

- Keep the opacity in felis's settings and only the blur in Cattery's file.
  It keeps to the first record, but what makes a window translucent and what
  blurs through it sit in two files.
- Keep the blur in felis's settings under a key of Cattery's own. felis's
  reader reports it as unknown every time the file is read.

## Consequences

- A window's look is in Cattery's own file: the themes, the opacity and the
  blur.
- An opacity set in felis's settings changes nothing in Cattery.
- If felis takes a blur strength, the opacity and the blur may move back into
  felis's settings with a new record.
