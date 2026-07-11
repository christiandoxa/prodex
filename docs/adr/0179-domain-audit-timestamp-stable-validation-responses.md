# 0179: Domain Audit Timestamp Stable Validation Responses

## Status

Accepted

## Context

Immutable audit events need trustworthy occurrence timestamps for ordering,
retention, incident response, and regulated exports. Existing compatibility
paths use raw `u64` Unix milliseconds. Without a validated path, adapters can
write zero, pre-Unix-millisecond, or implausibly far-future values that break
audit ordering and retention logic.

The serialized audit event shape must remain compatible, but new gateway and
control-plane boundaries need a typed timestamp and a redacted rejection plan.

## Decision

`prodex-domain` owns `AuditTimestamp`, `AuditEvent::new_at`, and
`plan_audit_timestamp_error_response`.

Validated audit timestamps are Unix milliseconds from `1_000` through
`4_102_444_800_000`. Validation failures map to stable status/code/message
response plans that do not echo rejected timestamps, tenant identifiers,
principal identifiers, resource identifiers, audit chain material, or
credential-like material.

## Consequences

- Gateway and control-plane adapters can reject invalid audit event timestamps
  before persistence or API exposure.
- Existing serialized audit events continue to store `occurred_at_unix_ms` as a
  numeric Unix millisecond field.
- Raw invalid timestamp values remain available only to trusted diagnostics.
