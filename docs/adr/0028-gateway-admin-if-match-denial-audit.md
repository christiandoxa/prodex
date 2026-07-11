# ADR 0028: Audit gateway admin If-Match precondition denials without logging ETags

## Status

Accepted

## Context

Gateway admin key update and delete endpoints support `If-Match` to prevent stale
control-plane writes. A failed precondition returns `412 precondition_failed` before the
mutation handler runs. These stale-write denials are security- and integrity-sensitive in
multi-admin and multi-replica deployments because they can indicate replayed or
concurrent control-plane requests.

The client-supplied ETag value is request-controlled metadata and should not be persisted
verbatim in audit logs.

## Decision

Failed admin `If-Match` checks now append a `gateway_admin` audit event with action
`request_denied` and outcome `failure`. The event records the authenticated admin actor,
role, method, path, backend label, and reason `precondition_failed`. It deliberately omits
the supplied and current ETag values.

## Consequences

Operators can investigate rejected stale admin mutations without leaking user-controlled
request validators. HTTP response behavior remains compatible: stale writes still return
`412 precondition_failed` and successful guarded mutations are unchanged.
