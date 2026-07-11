# ADR 0134: Application control-plane audit error boundary

## Status

Accepted

## Context

`prodex-application` composes control-plane decisions with append-only audit
storage planning. Audit storage planning failures can include tenant identifiers,
SQL backend names, migration/DDL details, or storage-key mismatches. Those raw
errors are useful for trusted diagnostics but should not become admin API or
readiness response text.

## Decision

Add `plan_application_control_plane_audit_error_response` to
`prodex-application`. It maps `ApplicationControlPlaneAuditError` to a stable
`audit_storage_unavailable` response with a service-unavailable status and a
generic message.

The raw storage planning error remains available to trusted, redacted logs and
operator diagnostics.

## Consequences

Control-plane composition roots can report audit persistence failures
consistently without leaking SQL backend details, tenant IDs, storage-key
mismatches, or migration/DDL internals.
