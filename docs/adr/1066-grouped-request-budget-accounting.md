# ADR 1066: Grouped Request-Budget Accounting

## Status

Accepted.

## Context

Distributed RPM/TPM admission was active through Redis, but grouped request
budgets still depended on process-local usage snapshots. Two gateway replicas
could therefore admit requests against the same logical budget group.

## Decision

Derive one PostgreSQL storage scope as SHA-256 over the canonical, explicitly
framed tenant, budget, team, project, and user values. The tenant is included in
both the digest and the database key; virtual keys with the same canonical scope
therefore share a counter without exposing raw governance identifiers in the
storage-scope string.

Admin-created virtual keys retain their persisted UUIDv7. Policy-defined keys,
which have no database row, derive an internal RFC 9562 UUIDv8 from the tenant
and case-normalized key name. This identity is stable across replicas and is
used only for distributed storage/rate-limit addressing; secret material is
never an input.

PostgreSQL migration v2 adds the non-negative `request_count` column to
`prodex_budget_counters`. One serializable, idempotent reservation transaction
atomically checks and increments request count while enforcing the existing
token and cost limits, then creates the reservation and reserved ledger event.
An idempotent replay does not increment the counter again.

`request_count` is cumulative admission history. Reconciliation releases unused
token and cost reservations but never decrements the accepted request count.

The production multi-replica startup gate now accepts a topology with at least
two gateway replicas, PostgreSQL durable state, Redis coordination, and the
accounting-check requirement enabled. File, SQLite, Redis-only, single-replica,
or coordination-free declarations still fail closed.

## Consequences

- Grouped request budgets are enforced across virtual keys and gateway replicas.
- Two independently pooled repositories prove one winner for a shared grouped
  request budget against real PostgreSQL.
- Two gateway proxies using two keys in one group prove one upstream request,
  one denial, one durable reservation, and one cumulative request count against
  real PostgreSQL.
- The heavy PostgreSQL proof runs both repository and active two-proxy checks.
- PostgreSQL TLS is not solved by this decision; production transport security
  remains a separate readiness requirement.

## Verification

```bash
npm run ci:storage-postgres-proof
npm run ci:storage-boundary-guard
```
