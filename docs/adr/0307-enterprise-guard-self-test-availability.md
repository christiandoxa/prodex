# ADR 0307: Enterprise Guard Self-Test Availability

## Status

Accepted

## Context

Enterprise guards protect boundary, storage, gateway, documentation, binary, and
deployment-security contracts. Most of these guards include `--self-test`
negative cases, but the enterprise docs guard only checked that the commands
were wired through CI and npm metadata.

## Decision

`scripts/ci/enterprise-docs-guard.mjs` now resolves every required enterprise
`ci:*` package script to its `scripts/ci/*.mjs` file and requires that guard
source to expose `--self-test`. Its own self-test rejects non-guard command
shapes.

## Consequences

- Enterprise guard wiring now also requires each guard to keep an executable
  self-test surface.
- Renaming a guard script to a non-Node or non-`scripts/ci` command fails the
  docs guard before CI silently loses negative coverage.
- This remains static CI validation only.
