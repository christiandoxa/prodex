# ADR 0309: Enterprise Docs Guard Self-Test Default

## Status

Accepted

## Context

The enterprise docs guard validates enterprise evidence documents, CI workflow
guard coverage, package scripts, changed-test aliases, and guard self-test
availability. It exposed `--self-test`, but the normal CI path did not execute
those guard-function negative fixtures unless callers remembered to run the
explicit mode separately.

## Decision

`scripts/ci/enterprise-docs-guard.mjs` now runs its self-tests before normal
workspace validation. The explicit `--self-test` mode remains available for
focused guard development and exits before scanning repository files.

## Consequences

- The standard `npm run ci:enterprise-docs-guard` path exercises the guard's own
  negative fixtures.
- CI and local preflight validate both the guard implementation and the current
  repository state with one command.
- This remains static CI validation only.
