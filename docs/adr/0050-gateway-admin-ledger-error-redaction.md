# ADR 0050: Redact gateway admin ledger load errors

## Status

Accepted

## Context

The gateway admin ledger and billing-summary endpoints load billing records from
state backends. File-backed deployments can fail to read the ledger because of
operator mistakes, storage corruption, or filesystem layout problems. Returning
the raw loader error to an authenticated admin response can disclose local paths,
backend implementation details, or parser/IO diagnostics that are not required
for API clients and are risky in regulated multi-tenant environments.

## Decision

Admin ledger load failures now return stable machine-readable errors with
redacted messages:

- `gateway_billing_ledger_load_failed`: `gateway billing ledger could not be loaded`
- `gateway_billing_summary_load_failed`: `gateway billing summary could not be loaded`

The detailed backend error remains an operational concern for diagnostics, not an
API response contract. This keeps the admin API secure-by-default while retaining
stable error codes for clients.

## Consequences

- Admin API callers receive deterministic responses that do not expose filesystem
  paths or low-level IO details.
- Regression coverage verifies that ledger, ledger CSV, summary, and summary CSV
  endpoints do not leak the ledger path, local root, token, or raw directory IO
  message when the file-backed ledger cannot be read.
- Operators should rely on runtime diagnostics/logs for backend-specific failure
  investigation instead of API response bodies.
