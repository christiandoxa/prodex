# ADR 1065: Atomic Gateway Secret Rotation

## Status

Accepted.

## Context

ADRs 1058 and 1059 made production credentials available through typed
`SecretRef` values and a Kubernetes projected volume, but a running gateway
kept the startup values. Reloading individual fields would allow one request to
observe mixed credential generations, and a Kubernetes `..data` update could
otherwise pair a value from one generation with a version sidecar from another.

## Decision

The projected provider resolves `..data` once and reads both the secret value
and optional `.version` sidecar beneath that canonical generation. Direct-file
mounts retain the existing contained-path fallback.

When a projected root is configured, the gateway runs a background refresh
every five seconds. Each pass resolves and validates the complete launch
candidate off the request path and computes an in-memory SHA-256 fingerprint of
the resolved secret contexts and values. An unchanged fingerprint is a no-op.
The provider bridge kind cannot change during refresh.

A valid changed candidate is published as one immutable credential snapshot
through `ArcSwap`. Each HTTP request and each Gemini Live connection pins one
snapshot for its full lifetime, so in-flight work keeps its original
credentials while new work observes the replacement. The snapshot includes
provider credentials, data-plane and admin authentication, SSO proxy
authentication, policy virtual keys, guardrail webhook authentication, and
observability HTTP authentication.

Policy virtual keys are rebuilt from the candidate and merged under the same
update lock with current admin-created keys. Policy keys win case-insensitive
name collisions, while unrelated admin keys remain available.

Resolution or validation failure leaves the previous snapshot active. Runtime
logs emit only the categorical `gateway_secret_refresh` outcomes `applied`,
`resolution_failed`, or `validation_failed`; secret values, references,
versions, paths, fingerprints, and underlying errors are not logged.

## Consequences

- Kubernetes value and version reads cannot cross a projected generation.
- Rotation never performs filesystem I/O in a request or streaming hot path.
- Existing requests and Gemini Live connections may use the previous
  credential until they finish; they are not forcefully reauthenticated.
- PostgreSQL and Redis connection URLs are restart-required. Refresh revalidates
  their references but excludes them from the live-credential fingerprint and
  does not rebuild or swap repositories, pools, or Redis executors.
- Exact version-pinned `SecretRef` values do not follow a new projected version
  until policy changes. Failed refreshes retain last-known-good indefinitely;
  the fixed five-second loop currently has no jitter, adaptive backoff, stale
  expiry, or readiness degradation.

## Verification

- `crates/prodex-secret-store/tests/projected_secret_provider.rs` covers atomic
  projection rotation, single-generation value/version anchoring, and
  generation escape rejection.
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_credentials.rs`
  covers atomic replacement, existing-snapshot pinning, validation failure
  last-known-good behavior, and policy/admin virtual-key merge preservation.
- `crates/prodex-app/tests/src/app_commands/runtime_launch/projected_secrets.rs`
  covers production resolution, secret fingerprint changes, and rejection of
  raw production credentials. Gemini Live uses the same connection-start
  pinning boundary, but does not yet have a dedicated end-to-end rotation test.
