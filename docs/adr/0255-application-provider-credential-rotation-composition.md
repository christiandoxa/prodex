# ADR 0255: Application Provider-Credential Rotation Composition

## Status

Accepted

## Context

Provider credential rotation is a security-sensitive control-plane operation.
Previous slices made the operation explicit in authorization and added durable
`SecretRef` storage plans, but adapters still needed to coordinate
authorization, append-only audit, and provider credential reference persistence.
That coordination is easy to get wrong in HTTP handlers, especially for denied
requests that must still be audited.

## Decision

Add `plan_application_provider_credential_rotation` to `prodex-application`.
The planner accepts the durable store, a `ProviderCredentialRotate`
control-plane action, a tenant-scoped `ProviderCredentialReferenceCommand`, and
audit digests. It rejects non-rotation operations, wrong resource kinds, and
cross-tenant action/reference pairs before selecting storage.

The planner always emits append-only audit storage for the authorization
decision. It emits provider credential reference storage only when the
control-plane decision is authorized, so denied rotations are auditable without
mutating credential references.

## Consequences

Control-plane adapters can call one application planner for provider credential
rotation instead of manually ordering authorization, audit, and storage writes.
Client-facing errors remain redacted and do not expose tenant IDs, provider
names, or secret reference paths.
