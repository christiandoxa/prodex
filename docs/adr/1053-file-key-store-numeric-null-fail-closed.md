# ADR 1053: File key-store numeric limits reject null

## Status

Accepted.

## Context

Admin-managed virtual keys can persist optional numeric budget and rate-limit
fields. SQL and Redis key-store loaders already reject malformed numeric fields,
but the file compatibility store used `Option<u64>` deserialization, which
treated explicit JSON `null` the same as an omitted legacy field.

Affected symbols:

- `RuntimeGatewayStoredVirtualKey::budget_microusd`
- `RuntimeGatewayStoredVirtualKey::request_budget`
- `RuntimeGatewayStoredVirtualKey::rpm_limit`
- `RuntimeGatewayStoredVirtualKey::tpm_limit`

The risk is fail-open policy drift: a corrupted present numeric field can
silently become absent and remove a persisted budget or rate-limit guard.

## Decision

The file store keeps omitted numeric fields compatible with legacy rows, omits
absent numeric fields when saving, and rejects explicit `null`, non-integer,
negative, or otherwise non-`u64` values during deserialization.

## Consequences

Legacy rows that omit budget and rate-limit fields still load. Rows with
present malformed numeric fields now fail the file-store load instead of
silently disabling budget or rate-limit policy.

Regression coverage:

- `load_store_rejects_null_virtual_key_numeric_fields`
- `saved_store_omits_absent_present_only_key_fields`
