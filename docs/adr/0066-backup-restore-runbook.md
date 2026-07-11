# ADR 0066: Gateway backup and restore runbook

## Status

Accepted.

## Context

The enterprise target requires backup/restore documentation and drills for the
gateway's durable state. The repository had deployment notes but no backend-aware
runbook that covered file, SQLite, PostgreSQL, Redis, audit logs, acceptance
checks, and restore evidence.

## Decision

Add `docs/backup-restore.md` as the canonical gateway backup and restore runbook
and guard its presence in the deployment security check. The runbook documents
backend-specific backup/restore commands, safe restore sequencing, and drill
acceptance criteria. PostgreSQL is identified as the durable source of truth for
multi-replica deployments; Redis restore guidance is limited to rebuildable or
explicitly accepted state.

## Consequences

- Operators have a concrete recovery procedure to execute and audit.
- Future storage/migration work must update the runbook and keep the guard green.
- This does not prove backup/restore has been drilled in every target
environment; it establishes the repository artifact required before those drills.
