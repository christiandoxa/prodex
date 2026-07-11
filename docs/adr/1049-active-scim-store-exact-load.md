# ADR 1049: Active SCIM store loads exact authorization rows

## Status

Accepted.

## Context

SCIM users are loaded from file, SQLite, PostgreSQL, or Redis compatibility
state and then served to admin and SSO/OIDC authorization paths. ADR 1046
filtered malformed rows before SSO/OIDC auth lookup, but the shared active
store load still returned corrupt persisted SCIM rows to other admin surfaces.

Affected symbols:

- `runtime_gateway_virtual_key_store_load`
- `RuntimeGatewayVirtualKeyStoreFile::canonicalize_for_active_state`

The risk is control-plane drift: a manually corrupted SCIM row can appear as
active runtime state even though admin writes and authentication would reject
the same authorization fields.

## Decision

Every active key-store load canonicalizes SCIM users through the exact stored
SCIM authorization boundary. Rows with malformed principal, role, governance,
or key-prefix authorization fields are omitted from active state.

## Consequences

Canonical SCIM users continue to load. Corrupt compatibility rows no longer
appear in active admin state, while persisted data remains available for manual
repair or migration tooling.

Regression coverage:

- `key_store_load_filters_malformed_scim_rows_from_active_state`
