# ADR 1058: Projected External Secret Provider

## Status

Accepted.

## Context

The domain defines `SecretProvider`, but production code had no implementation
for secrets synchronized from an external manager. Kubernetes `ExternalSecret`
currently materializes credentials as environment variables, which cannot
rotate in a running process. A cloud-vendor SDK in the domain or gateway would
couple credential resolution to one provider and introduce network I/O into
runtime paths.

## Decision

Add `ProjectedSecretProvider` in `prodex-secret-store`. It reads a read-only
directory populated by External Secrets, Secrets Store CSI, or an equivalent
workload-identity adapter. The provider canonicalizes the root and each file,
rejects traversal and symlink escape, accepts only regular non-world-readable
files, bounds secret and version sizes, and maps filesystem failures to the
domain's redacted resolution errors.

An optional `<name>.version` sidecar carries the manager version. Exact requested
versions fail with `StaleVersion` when the sidecar is absent or different. For
Kubernetes-style projections, each resolution anchors the value and version
reads to one canonical `..data` generation, so an atomic projection update is
visible without process restart and cannot produce a mixed pair. Callers must
resolve during startup or bounded background refresh, not on the async request
hot path.

## Consequences

- The adapter remains vendor-neutral and uses no provider SDK or network client.
- Standard Kubernetes symlink-based atomic projections work when their resolved
  targets remain inside the mounted root.
- Gateway policy/composition-root wiring and atomic last-known-good background
  refresh are implemented by ADRs 1059 and 1065.
- Environment/file compatibility remains development-only once production mode
  enforcement is wired.
