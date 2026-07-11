# ADR 1052: File key-store disabled flags reject null

## Status

Accepted.

## Context

Admin-managed virtual keys persist a `disabled` boolean. SQL and Redis key-store
loads already reject malformed boolean fields, but the file compatibility store
used `Option<bool>` deserialization, which treated an explicit JSON `null` the
same as a missing legacy field.

Affected symbols:

- `RuntimeGatewayStoredVirtualKey::disabled`
- `runtime_gateway_virtual_key_store_file_load`

The risk is fail-open state drift: a corrupted persisted `"disabled": null`
field can become `None` and then load as an enabled key through the active
runtime conversion default.

## Decision

The file store keeps missing `disabled` compatible with legacy rows, omits
absent `disabled` values when saving, and rejects explicit `null` or
non-boolean values during deserialization.

## Consequences

Legacy rows that omit `disabled` still load. Corrupt rows with a present but
non-boolean disabled field now fail the file-store load instead of silently
enabling a key.

Regression coverage:

- `load_store_rejects_null_virtual_key_disabled_field`
- `saved_store_omits_absent_present_only_key_fields`
