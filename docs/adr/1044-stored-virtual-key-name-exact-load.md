# ADR 1044: Stored virtual-key names load exactly

## Status

Accepted.

## Context

Admin-managed virtual-key records can be loaded from file, SQLite, PostgreSQL,
or Redis compatibility state. Admin write paths already reject padded or
whitespace-bearing key names, but the shared stored-key conversion still trimmed
persisted names before placing records into active runtime state.

Affected symbols:

- `runtime_gateway_virtual_key_entry_from_stored`
- `runtime_gateway_virtual_key_entries_from_sources`

The risk is authorization and accounting drift: a corrupt persisted key named
`" alpha "` could become the active key `alpha`, bypassing the exact admin
request boundary.

## Decision

Stored virtual-key names must pass the same exact key-name validation used by
admin writes. Whitespace-bearing or otherwise invalid persisted key names no
longer become active runtime keys through trim-normalization.

## Consequences

Canonical stored key names are unchanged. Corrupt compatibility records are
ignored instead of becoming a different active key.

Regression coverage:

- `stored_key_rejects_padded_name`
