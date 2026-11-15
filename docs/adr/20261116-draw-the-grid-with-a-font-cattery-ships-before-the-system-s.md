# Draw the grid with a font Cattery ships before the system's

Status: Accepted

## Context

A grid drawn with the system's fonts looks different on macOS, Linux and
Windows, and none of their default fixed-width faces carries the icons
shell prompts draw. A fixed-width face for Latin text carries no Japanese,
so a font that replaced the system's would leave such text without glyphs.

## Decision

Cattery ships JetBrains Mono Nerd Font Mono inside its executable and draws
the grid with it unless the settings name another family. Its faces are
searched before the system's fonts, and whatever they lack is drawn with the
system's.

## Alternatives considered

- Keep drawing with the system's fonts. Windows keep looking different on
  each platform, and prompts' icons stay missing until the user installs a
  font that has them.
- Ship the font as files beside the executable. Each package and a
  development build would need its own way to find them, where an
  executable that carries them works alike everywhere.

## Consequences

- A new window looks the same on every platform, and prompts' icons show
  without anything installed.
- The executable is larger by the font's size.
- The licences of the font and of every icon set in it ship with Cattery.
- A family the settings name, installed or not, is searched in the shipped
  faces first.
