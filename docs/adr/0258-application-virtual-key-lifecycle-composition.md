# ADR 0258: Application Virtual-Key Lifecycle Composition

## Status

Accepted

## Context

Virtual key create and rotate-secret operations are control-plane key lifecycle
events. Storage plans now persist only `SecretRef` fields for virtual-key
secrets, but adapters still need a safe way to compose authorization,
append-only audit, and authorized-only storage mutation.

Without an application composition boundary, HTTP handlers could accidentally
write a secret reference before authorization, skip denied-request audit, or
mix `VirtualKeyCreate` requests with rotate-secret storage semantics.

## Decision

Add `plan_application_virtual_key_lifecycle` to `prodex-application`. The
planner accepts a `VirtualKeyCreate` or `VirtualKeyRotateSecret`
control-plane action, a matching `VirtualKeySecretReferenceCommand`, durable
store selection, and audit digests.

The planner rejects non-virtual-key operations, wrong resource kinds,
cross-tenant action/reference pairs, and create/rotate reference-kind
mismatches before selecting storage. It always emits append-only audit storage
for the authorization decision and emits virtual-key secret reference storage
only when the decision is authorized.

## Consequences

Control-plane adapters can route virtual-key lifecycle requests through one
side-effect-free application boundary. Denied operations remain auditable,
successful mutations persist only external secret references, and
client-visible errors do not expose tenant IDs or secret reference paths.
