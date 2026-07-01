# ADR 0306: Enterprise Guard Alias Command Match

## Status

Accepted

## Context

ADR 0305 required `scripts/ci/test-impact-manifest.json` to contain aliases for
every enterprise guard script. Presence alone does not prove the alias command
matches `package.json`; a stale alias could keep `package:changed-aliases` green
for the wrong command.

## Decision

`scripts/ci/enterprise-docs-guard.mjs` now compares every required enterprise
guard alias command in `scripts/ci/test-impact-manifest.json` against the
matching `package.json` script command. Its self-test rejects mismatched
commands.

## Consequences

- Enterprise package alias coverage now checks presence and exact command
  equality.
- Package script drift detection cannot silently validate a stale guard command.
- The change remains static CI metadata validation only.
