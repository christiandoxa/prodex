# ADR 0539: Gateway billing ledger exposes UUIDv7 request IDs

## Status

Accepted

## Context

Gateway billing ledger records retained the legacy numeric runtime request
sequence in the `request` field. That value is useful for local runtime logs but
is not globally unique across gateway replicas.

## Decision

Add an optional serialized `request_id` field to gateway billing ledger entries.
New file and Redis ledger entries populate it as `prodex-{RequestId}` using the
typed UUIDv7 domain ID. The existing numeric `request` field remains for
compatibility and local correlation.

## Consequences

Ledger JSON, CSV, and OpenAPI consumers have a globally unique request
identifier for new records. ADR 0541 adds tenant and governance scope snapshots
to the same new file/Redis ledger records. SQL-backed ledger storage still
needs a schema migration before it can persist and reload these fields.
