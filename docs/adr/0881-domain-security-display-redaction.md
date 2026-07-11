# ADR 0881: Domain security display redaction

## Status

Accepted.

## Context

Domain security response planners already expose stable messages for missing
tenant context, tenant access denial, and role-claim denial. The raw `Display`
implementations still identified missing tenant claims, missing principal
tenant context, cross-tenant access, and unknown role-claim classification.

## Decision

Render `TenantResolutionError`, `TenantAccessError`, and `RoleClaimError` with
the same low-cardinality messages used by their response planners. Keep typed
variants unchanged so callers can still classify failures for audit and metrics.

Regression coverage pins the exact display strings while preserving existing
checks that tenant IDs, role names, scopes, and role-claim values do not appear
in responses or debug output.

## Consequences

Security boundaries can safely fall back to raw display strings without
revealing which claim or tenant-isolation rule failed. Trusted diagnostics
should continue using typed variants.
