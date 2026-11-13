# Keep Cattery's settings in felis's format in a file of its own

Status: Accepted

## Context

Cattery's settings — font, colours, cursor, the shell a session runs — are
changed in its Preferences window and kept in a file, which a user may also
edit by hand. felis already defines these settings, reads them from a TOML
file, and checks and reports what it reads; its renderer and daemon take them
in that shape. felis keeps that file for its own windows.

## Decision

Cattery keeps its settings in a file of its own, written in felis's
configuration format and read with felis's own reader.

## Alternatives considered

- A format of Cattery's own. Every setting would be defined twice and
  translated into felis's, and each new felis setting would wait for Cattery
  to define it.
- felis's own file. Cattery and felis would share one set of settings, so a
  change in Cattery's Preferences window would also change felis's windows.

## Consequences

- A setting felis adds can be used in Cattery's file with the next felis pin,
  before the Preferences window shows it.
- What felis's format does not have, Cattery's file cannot hold; a setting
  Cattery wants beyond it is asked of felis.
- A felis file can be copied over to start Cattery's.
- The format changes when felis changes it, as the felis pin moves.
