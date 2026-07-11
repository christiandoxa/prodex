# ADR 1059: Production Gateway SecretRef Wiring

## Status

Accepted.

## Context

ADR 1058 added a vendor-neutral projected external-secret adapter, but gateway
startup still consumed provider, authentication, state-store, SSO,
observability, and webhook credentials from CLI arguments or environment
variables. Kubernetes also injected the Secret through `envFrom`, so projected
updates were unavailable to the provider contract.

## Decision

Add explicit `secrets.production`, `secrets.projected_root`, and
`secrets.projected_provider` policy settings. Gateway credential fields accept
typed `SecretRef` values. Production policy requires authentication and a
projected upstream provider key, rejects CLI and environment credential
sources, requires every reference to use the configured provider, and resolves
all credential material in the application composition root before request
serving.

The Kubernetes workload mounts the ExternalSecret-backed Secret read-only at
`/run/secrets/prodex` as a projected volume with mode `0440`, without `subPath`
or Secret `envFrom`. ConfigMap environment input remains non-secret.

## Consequences

- Existing CLI/environment credential behavior remains available when
  `secrets.production=false`.
- Production startup fails closed on missing, malformed, stale, foreign, or
  unreadable secret references without logging material or reference names.
- Request handling performs no secret filesystem I/O.
- The projected provider observes atomic rotation when resolved again. A
  bounded background refresh that atomically swaps live gateway credentials is
  still required for rotation of already-started gateway state.
