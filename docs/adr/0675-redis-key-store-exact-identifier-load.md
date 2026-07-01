# ADR 0675: Redis Key Store Loads Identifiers Exactly

## Status

Accepted.

## Context

The gateway Redis key store stores virtual-key and SCIM user fields in Redis
hashes. Admin write paths already reject padded key names, scope IDs, SCIM
principals, roles, and budget IDs, but the Redis load path trimmed hash string
fields before rebuilding the in-memory store.

That normalization can turn a corrupted or manually written padded Redis hash
field into a canonical key, scope, or SCIM principal at runtime.

## Decision

Redis key-store loading now treats identity and governance fields as exact
non-empty strings without whitespace. Redis-loaded identifier arrays such as
`allowed_models` and `allowed_key_prefixes` must also contain only exact
non-empty strings without whitespace. Display-oriented SCIM strings such as
`displayName` and `externalId` keep the existing compatibility normalization.

## Consequences

Padded Redis hash identifiers and identifier-array entries are ignored instead
of normalized into active control-plane or data-plane state. Canonical Redis
key-store records are unchanged.
