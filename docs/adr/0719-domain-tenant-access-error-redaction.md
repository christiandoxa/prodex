# ADR 0719: Redact Tenant Access Error Details

## Status

Accepted

## Context

Tenant authorization failures are security-sensitive. `TenantAccessError` keeps
structured principal and resource tenant IDs so trusted callers can classify a
cross-tenant denial. Its `Display` and derived `Debug` strings still echoed both
tenant IDs.

## Decision

Keep the structured `TenantAccessError::CrossTenantAccess` fields, but make its
`Display` and `Debug` strings generic. Also make the tenant branch of
`ResourceAuthorizationError` debug output generic so wrapped resource
authorization failures do not re-expose tenant IDs. The stable response planner
continues returning `tenant_access_denied` with a fixed message.

## Consequences

- Principal and resource tenant IDs are not echoed through tenant-access
  `Display`, `Debug`, or wrapped resource-authorization `Debug` strings.
- Internal code can still match the structured variant for trusted diagnostics.
- Cross-tenant denial handling stays aligned with regulated error-redaction
  requirements.
