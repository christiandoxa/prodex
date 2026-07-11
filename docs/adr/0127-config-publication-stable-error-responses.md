# ADR 0127: Configuration publication stable error responses

## Status

Accepted

## Context

The revisioned configuration boundary rejects publication attempts when the
candidate belongs to a different tenant or is not newer than the active revision.
Raw `ConfigPublicationError` values include tenant identifiers and policy
revision identifiers. Those values are useful in trusted audit trails and
operator diagnostics, but should not be exposed in client-visible admin API
responses.

## Decision

Add `plan_config_publication_error_response` to `prodex-config`. It maps
configuration publication failures into stable status/code/message triples:

- tenant mismatches become `configuration_tenant_mismatch` with a bad-request
  status; and
- stale or same revision attempts become `configuration_revision_not_newer` with
  a conflict status.

The response message remains generic and excludes tenant IDs, policy revision
IDs, payload contents, and cache internals.

## Consequences

Control-plane composition roots can return deterministic, machine-readable
configuration publication failures while retaining the raw error for append-only
audit events and trusted redacted diagnostics.
