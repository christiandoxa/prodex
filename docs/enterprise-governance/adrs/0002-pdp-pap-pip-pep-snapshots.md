# ADR 0002: PDP, PAP, PIP, PEP, and Snapshots

- Status: Proposed
- Scope: target enterprise governance architecture

## Context

Policy administration, attribute collection, decision and enforcement must not
collapse into an untestable request handler or add storage/network work to the
runtime hot path.

## Decision

Separate responsibilities:

- PAP validates, versions and submits policy changes;
- PIP assembles authenticated, bounded attributes and revision identifiers;
- PDP is pure and deterministic over immutable `PolicyInput` plus a verified
  compiled snapshot; and
- PEPs enforce typed obligations at gateway, inspection, approval, routing,
  streaming, accounting and audit boundaries.

Background workers load, compile and checksum snapshots, then atomically swap
read-only references. The request path performs no compilation, broad storage
read, provider probe, SIEM, Vault, DNS or external PDP call. Invalid snapshots
never become active; a verified non-revoked LKG is mode-gated.

## Consequences

Snapshot and input revisions become part of decision/cache keys and audit.
Every mandatory obligation needs an acknowledging PEP. Revocation/invalidation
overrides cache and LKG.

## Implementation status

Policy caching, ArcSwap/LKG and application boundary primitives exist. The
unified governance PAP/PIP/PDP/PEP contract and full snapshot lifecycle remain
to be implemented.
