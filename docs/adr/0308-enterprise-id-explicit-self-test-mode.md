# ADR 0308: Enterprise ID Explicit Self-Test Mode

## Status

Accepted

## Context

The enterprise ID boundary guard ran its self-tests before scanning the
workspace, but it did not expose an explicit `--self-test` mode like the other
enterprise guards. ADR 0307 made self-test availability part of the enterprise
guard wiring contract.

## Decision

`scripts/ci/enterprise-id-boundary-guard.mjs` now accepts `--self-test`, runs
only its negative/positive guard fixtures, and exits before workspace scanning.
The normal guard path still runs those self-tests before scanning.

## Consequences

- Enterprise ID guard behavior now matches the explicit self-test interface used
  by the other enterprise guards.
- The enterprise docs guard can statically verify self-test availability across
  the required guard set.
- This is CI tooling only and does not affect runtime request behavior.
