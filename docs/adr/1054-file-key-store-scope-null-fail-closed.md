# ADR 1054: File key-store scope identifiers reject null

## Status

Accepted.

## Context

Admin-managed virtual keys can persist optional key identity and governance
scope fields. Active stored-key conversion already rejects whitespace-bearing
scope strings, but the file compatibility store used `Option<String>`
deserialization, which treated explicit JSON `null` the same as an omitted
legacy field.

Affected symbols:

- `RuntimeGatewayStoredVirtualKey::virtual_key_id`
- `RuntimeGatewayStoredVirtualKey::tenant_id`
- `RuntimeGatewayStoredVirtualKey::team_id`
- `RuntimeGatewayStoredVirtualKey::project_id`
- `RuntimeGatewayStoredVirtualKey::user_id`
- `RuntimeGatewayStoredVirtualKey::budget_id`

The risk is tenant or budget isolation drift: a corrupted present scope field
can silently become absent and remove a persisted key boundary.

## Decision

The file store keeps omitted identity and governance fields compatible with
legacy rows, omits absent values when saving, and rejects explicit `null` or
non-string values during deserialization.

## Consequences

Legacy rows that omit scope fields still load. Rows with present malformed
scope fields now fail the file-store load instead of silently disabling tenant,
team, project, user, or budget scope policy.

Regression coverage:

- `load_store_rejects_null_virtual_key_scope_fields`
- `saved_store_omits_absent_present_only_key_fields`
