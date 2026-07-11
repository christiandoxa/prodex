# ADR 1050: Stored virtual-key model scopes load exactly

## Status

Accepted.

## Context

Admin key create/update validation rejects empty or whitespace-bearing
`allowed_models` entries. SQL and Redis key-store rows also pass through exact
JSON-array validation, but file compatibility rows deserialize directly into
stored virtual-key records before conversion into active runtime keys.

Affected symbol:

- `runtime_gateway_virtual_key_entry_from_stored`

The risk is policy widening or drift: a manually corrupted file row can carry a
model allow-list value that admin writes and durable backend loads would reject.

## Decision

Stored virtual-key conversion now rejects any `allowed_models` entry that is
empty or contains whitespace before the key can become active.

## Consequences

Canonical allow-lists continue to load. Corrupt file compatibility rows are
ignored like other malformed stored virtual-key authorization fields instead of
becoming active model scopes.

Regression coverage:

- `stored_key_rejects_padded_allowed_model_scope`
