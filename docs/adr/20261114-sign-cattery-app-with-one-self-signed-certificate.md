# Sign Cattery.app with one self-signed certificate

Status: Accepted

## Context

macOS remembers what the user let an app do — post notifications, for
one — by the app's signature. An ad-hoc signature names nothing but the
build's own hash, so to macOS every release would be a different app, and
each update would ask again. Cattery has no Apple Developer ID.

## Decision

Every release of Cattery.app is signed by the same self-signed certificate,
"Cattery Release". Local bundles are signed by it too when the keychain has
it.

## Alternatives considered

- An Apple Developer ID. It would also let Gatekeeper open the app without
  the user's approval, but it costs a paid membership the project does not
  hold.
- Stay ad hoc. Every update would ask the user again.

## Consequences

- An update keeps what the user let the previous release do.
- Gatekeeper still does not trust the app, so installing it still needs the
  quarantine attribute removed or the user's approval.
- Losing or replacing the certificate makes every user grant the app its
  permissions once more.
