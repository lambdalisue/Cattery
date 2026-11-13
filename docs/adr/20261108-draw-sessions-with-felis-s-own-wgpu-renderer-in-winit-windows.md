# Draw sessions with felis's own wgpu renderer in winit windows

Status: Accepted

## Context

Cattery runs on macOS, Linux and Windows, and a session should look and
respond in Cattery the way it does in felis's own window. felis's window
draws a session with a wgpu renderer that takes any window handle, together
with the font shaping and screen painting it is built on. Cattery starts as
one window per session, with a Preferences window beside it; tabs and
widgets come later.

## Decision

Cattery's windows are winit windows, and each session is drawn by felis's
wgpu renderer. The Preferences window is drawn with egui on the same wgpu.

## Alternatives considered

- GPUI. It is a full UI framework, which tabs and widgets would want, but
  every part of drawing a terminal — images, overlays, shaders — would be
  written again, and it is pinned to the Zed repository with no stable
  release.
- Native AppKit with Metal. It is macOS only.
- A web view (Dioxus, Tauri). The terminal would be drawn by the page, which
  costs weight and fidelity in exactly the window Cattery is about.
- The Preferences window in each platform's native toolkit. Three
  implementations of the same settings.

## Consequences

- A session in Cattery draws what felis draws, and improvements to felis's
  renderer reach Cattery with the next pin.
- Window chrome beyond the session — tab bars, widgets — has no UI toolkit
  under it in a session window. Adding one may take felis's renderer drawing
  into part of a window, which is asked of felis.
- The Preferences window does not look native on any platform.
