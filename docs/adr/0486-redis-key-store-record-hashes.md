# ADR 0486: Store Redis Gateway Keys as Record Hashes

## Status

Accepted.

## Context

The Redis gateway key-store backend persisted the full virtual-key store as one JSON value. That shape made Redis a whole-map state container and required every admin mutation to rewrite unrelated records.

Enterprise deployments need Redis usage to stay limited to cache and coordination roles, with durable state moving toward database-backed records. The existing Redis backend still needs a compatible migration path for existing installs.

## Decision

The Redis key-store backend now stores virtual keys and SCIM users as per-record Redis hashes. Index sets track the record names:

- `<store>:keys`
- `<store>:key:<name>`
- `<store>:scim_users`
- `<store>:scim_user:<id>`

Reads fall back to the legacy whole-store JSON value when the new key index is absent. Saves delete the legacy whole-store value and rewrite the indexed record hashes.

## Consequences

Redis no longer persists the gateway key store as a single whole-map JSON blob. Existing legacy Redis stores are still readable and are converted on the next successful save.

The current admin mutation lock remains in place to preserve existing mutation semantics. Replacing that lock with finer-grained database-backed concurrency control is future work.
