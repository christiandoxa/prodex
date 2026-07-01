# 0206: Application Audit Retention Purge Audit Storage

## Status

Accepted

## Context

`ControlPlaneOperation::AuditRetentionPurge` authorizes audit log retention
deletes and emits a control-plane audit event. The application boundary must
preserve that audit event in durable append-only audit storage for both
authorized and denied attempts.

If composition roots invoke retention purge storage without this audit path,
security-sensitive audit log deletes can happen without the immutable audit
record required by the enterprise control-plane model.

## Decision

Route audit retention purge control-plane decisions through
`plan_application_control_plane_with_audit_storage`.

Authorized purge operations select append-only audit storage with the stable
`control_plane.audit.retention_purge` action before concrete purge execution is
adapted. Denied purge attempts still select append-only audit storage and record
the denial outcome. The separate `plan_application_audit_retention_purge`
storage use case remains responsible for the tenant-scoped SQL delete plan.

## Consequences

- Retention purge attempts are durably auditable at the application boundary.
- Denied purge attempts do not disappear before persistence planning.
- Composition roots have separate plans for audit append and purge delete.
- Audit storage failures remain behind the existing redacted audit-storage
  application error boundary.
