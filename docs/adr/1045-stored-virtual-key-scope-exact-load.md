# ADR 1045: Stored virtual-key governance scopes load exactly

## Status

Accepted.

## Context

Admin-managed virtual-key records can carry tenant, team, project, user, and
budget scope identifiers. Admin write paths already reject whitespace-bearing
scope identifiers, but the shared stored-key conversion previously copied
persisted scope fields directly into active runtime state.

Affected symbol:

- `runtime_gateway_virtual_key_entry_from_stored`

The risk is tenant or budget isolation drift: a corrupt persisted scope such as
`" tenant-a "` could become active runtime state even though the admin API would
reject that value.

## Decision

Stored virtual-key governance scope identifiers must be absent or exact
non-empty values without whitespace. Whitespace-bearing persisted scopes no
longer become active runtime keys.

## Consequences

Canonical stored scopes are unchanged. Empty compatibility fields still mean
absent scope; corrupt padded scopes are ignored with the stored key record
instead of being normalized or accepted.

Regression coverage:

- `stored_key_rejects_padded_governance_scope`
