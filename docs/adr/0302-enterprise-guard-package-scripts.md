# ADR 0302: Enterprise Guard Package Scripts

## Status

Accepted

## Context

ADR 0301 made the enterprise docs guard verify that GitHub Actions still runs
the enterprise guard commands. That check would not catch a missing `package.json`
script until the workflow executed, because the workflow stores commands as
`npm run ci:*`.

## Decision

`scripts/ci/enterprise-docs-guard.mjs` now validates that `package.json` exposes
every enterprise `ci:*` script used by the guarded workflow. Its self-test
rejects a package manifest missing an enterprise guard script.

## Consequences

- CI workflow coverage and npm script availability are checked together.
- Removing or renaming an enterprise guard script fails during the fast docs
  guard instead of surfacing later as a workflow command error.
- The check remains static Node tooling and does not touch runtime request
  behavior.
