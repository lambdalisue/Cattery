# Keep every session in a felis daemon of cattery's own

Status: Accepted

## Context

cattery is a felis client: its sessions live in a felis daemon, which holds
their pseudo-terminals, emulator state, scrollback and images while windows
come and go. felis is not published and promises no API stability, so a
client and a daemon built from different revisions may not understand each
other.

## Decision

cattery starts a felis daemon itself, from the felis it is built with, on a
socket of its own, and keeps every session it shows in that daemon.

## Alternatives considered

- Attach to the felis daemon the user may already run. Sessions could then be
  shared with the felis command line, but that daemon's revision is not
  cattery's, and cattery would depend on felis being installed.
- Attach to the user's daemon when one is running and start its own
  otherwise. It carries the costs of both, and which daemon a session lands in
  would depend on what happened to be running.

## Consequences

- Installing felis is not a prerequisite for cattery.
- Sessions started from the felis command line against its default socket do
  not appear in cattery, and the reverse.
- felis is pinned to one git revision; moving the pin may need cattery
  changed.
- A daemon left running by an older cattery may speak an older protocol than
  a newer cattery.
