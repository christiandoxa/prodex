# ADR 0301: Enterprise Workflow Guard

## Status

Accepted

## Context

ADR 0300 moved enterprise boundary, storage, gateway, documentation, binary, and
deployment security guards into the GitHub Actions `process-guard` job. That
workflow coverage must not drift independently from the enterprise audit and
preflight commands.

## Decision

`scripts/ci/enterprise-docs-guard.mjs` now verifies that `.github/workflows/ci.yml`
contains the enterprise boundary guard step and every required guard command.
Its self-test proves that a workflow missing an enterprise guard is rejected.

## Consequences

- The enterprise docs guard now protects both documentation evidence and CI
  workflow coverage for the same release gates.
- CI cannot silently drop a boundary/deployment guard while leaving the audit
  prose unchanged.
- The check is static Node tooling and does not add runtime or request-path
  behavior.
