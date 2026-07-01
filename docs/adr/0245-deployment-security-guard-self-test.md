# ADR 0245: Deployment security guard self-test

## Status

Accepted

## Context

The deployment security guard protects production-shaped Compose, Docker, and
Kubernetes artifacts from insecure regressions. After adding explicit
Kubernetes ServiceAccounts, the guard itself needed negative coverage so missing
workload identity, default service accounts, enabled token automount, static
credentials, and defaulted gateway tokens are proven to fail.

## Decision

`scripts/ci/deployment-security-guard.mjs` now exposes a pure
`validateDeploymentSecurity` function and a `--self-test` mode. The self-test
uses in-memory fixtures to prove that:

- a complete hardened fixture passes;
- defaulted gateway tokens fail;
- `change-me` credentials fail;
- missing gateway service-account bindings fail;
- the default Kubernetes service account fails; and
- enabled service-account token automount fails.

## Consequences

The guard remains usable in CI exactly as before, while its own security
assumptions can be validated without mutating repository files. Future
deployment-hardening checks should add both the production marker and a negative
self-test fixture.
