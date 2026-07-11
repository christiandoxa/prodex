# ADR 1057: PostgreSQL Backup and Restore Drill

## Status

Accepted.

## Context

The backup runbook described PostgreSQL recovery but did not execute it or prove
RPO, RTO, tenant isolation, and accounting integrity. Documentation-only checks
cannot detect an unrestorable dump, missing tenant tables, duplicate ledger
rows, or lost RLS policy.

## Decision

Add a Docker-backed CI drill using PostgreSQL 16. The drill invokes the external
`prodex-gateway migrate` command, seeds two synthetic tenants across all twelve
tenant-owned tables, creates a custom-format dump, writes a post-backup marker,
and restores into a new database.

The restored database must match the pre-backup fingerprint, exclude the later
marker, preserve accounting totals and ledger uniqueness, and enforce read and
write isolation on all tenant-owned tables through a non-owner `NOBYPASSRLS`
role. Recovery-point age and restore duration must remain within configurable
RPO and RTO thresholds.

The job uploads a redacted JSON evidence artifact containing only the source
revision, artifact digest and size, timings, bounded counts, invariant results,
and stable failure codes. Database URLs, credentials, container identifiers,
SQL dumps, and tenant identifiers are excluded.

## Consequences

- Every heavy CI run proves that the current migration can be dumped and
  restored with tenant/accounting invariants intact.
- The drill remains synthetic; operators must still execute the runbook against
  each production backup facility and retention policy.
- PostgreSQL is the durable recovery target. Redis remains rebuildable cache,
  rate-limit, and coordination state rather than billing source of truth.
