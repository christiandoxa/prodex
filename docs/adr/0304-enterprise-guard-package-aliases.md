# ADR 0304: Enterprise Guard Package Aliases

## Status

Accepted

## Context

ADR 0302 made the enterprise docs guard check that `package.json` exposes every
enterprise `ci:*` guard script used by CI. The changed-test package alias
manifest still pinned only the older enterprise ID guard alias, so a package
script command could drift without the existing `package:changed-aliases` check
catching it.

## Decision

`scripts/ci/test-impact-manifest.json` now pins all enterprise guard package
aliases used by the CI workflow and preflight guard set.

## Consequences

- `package:changed-aliases` validates enterprise guard command strings when
  `package.json` changes.
- The workflow guard, package script availability guard, and package alias guard
  now cover the same enterprise guard set.
- This remains CI metadata only and does not affect runtime request behavior.
