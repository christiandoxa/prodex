# 0166: Audit Stable Error Responses

## Status

Accepted

## Context

Enterprise deployments require immutable audit events for security-sensitive
actions and append-only hash-chain verification for durable audit persistence.
Audit digest validation and chain-link verification failures can include
previous/current digest values, tenant context, principal identifiers, resource
IDs, action names, or storage topology when surfaced through adapter errors.

Those details are useful in trusted diagnostics, but client-visible API
responses must not reveal audit-chain internals or security-event topology.

## Decision

`prodex-domain` owns stable audit response planners:

- `plan_audit_digest_error_response`
- `plan_audit_chain_error_response`

Digest validation failures map to `audit_digest_invalid`. Chain-link conflicts
map to `audit_chain_conflict`. Both planners return small status/code/message
response plans and deliberately omit digest values, tenant IDs, principal IDs,
resource IDs, action names, and storage-chain internals.

## Consequences

- Control-plane and storage adapters can expose audit persistence/verification
  failures through one stable redacted boundary.
- Append-only audit diagnostics remain available internally without becoming a
  public API contract.
- Security-sensitive mutation paths can fail closed without leaking audit-chain
  metadata.
