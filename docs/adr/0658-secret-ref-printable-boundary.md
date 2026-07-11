# 0658. SecretRef Printable Boundary

## Status

Accepted

## Context

Enterprise configuration stores provider credentials through `SecretRef`
references rather than raw secret strings. The existing shape guard rejected
empty and whitespace-containing reference parts, but it still allowed
non-printable, non-ASCII, or unbounded provider, name, and version values.

Those values can create ambiguous secret-manager lookups, unstable audit
evidence, and unsafe log or response rendering if an adapter mishandles them.

## Decision

`SecretRef::is_well_formed()` now accepts only non-empty printable ASCII
provider, name, and version parts with a 128-byte maximum per part.

The constructor remains source-compatible. Validation stays at the publication
or provider-boundary call sites that already call `is_well_formed()`.

## Consequences

- Secret references remain structured references, not raw secret material.
- Control characters, Unicode, whitespace, and overlong parts are rejected
  before config publication planning.
- Existing well-formed references using printable ASCII continue to work.
