# ADR 0541: Gateway billing ledger records scope snapshots

## Status

Accepted

## Context

Gateway billing summaries derived tenant and governance dimensions from the
current virtual-key configuration. Historical billing rows could therefore be
reported under the wrong tenant, team, project, user, or budget after a key was
edited, deleted, or recreated.

## Decision

New gateway usage deltas carry the virtual key's tenant and governance scope at
admission time. File and Redis billing ledger entries serialize those optional
scope fields, and billing summaries prefer the ledger snapshot before falling
back to current key configuration for older records.

## Consequences

New file and Redis billing rows keep stable historical scope for admin JSON,
CSV, and summary views. SQL-backed gateway billing rows still require a
versioned migration before those scope snapshots can be persisted and reloaded.
