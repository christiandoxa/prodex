# ADR 0063: Compose secure deployment defaults

## Status

Accepted.

## Context

The enterprise deployment target forbids production-shaped compose examples that
ship with fallback credentials such as `change-me` or static database passwords.
The gateway container scaffold must also reflect the hardened runtime posture
expected from Kubernetes manifests: non-root execution, read-only root
filesystem, dropped Linux capabilities, and no-new-privileges.

## Decision

Require `PRODEX_GATEWAY_TOKEN` explicitly in `compose.yaml`, remove static
Postgres password defaults, blank out example secret values, and harden the
gateway service with a read-only filesystem, dropped capabilities,
`no-new-privileges`, and a bounded noexec `/tmp` tmpfs. Add a CI guard that
prevents reintroducing static credential defaults or removing the baseline
container hardening.

## Consequences

- Local compose users must provide real secrets before startup instead of
  accidentally exposing a known token.
- The compose artifact is closer to the production/Kubernetes security posture.
- Optional Postgres profile users must set `PRODEX_POSTGRES_PASSWORD` and keep
  `PRODEX_GATEWAY_POSTGRES_URL` aligned with that secret.
