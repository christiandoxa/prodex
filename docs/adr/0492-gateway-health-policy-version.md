# ADR 0492: Expose Gateway Policy Version In Health Probes

## Status

Accepted.

## Context

Enterprise operators need `/livez`, `/readyz`, and `/startupz` to report which policy snapshot a gateway process activated. Reading policy from disk on every probe would add request-path I/O and could make health checks depend on transient filesystem state.

## Decision

The local rewrite gateway captures the loaded runtime policy version once during proxy startup and includes it as `policy_version` in operational health probe JSON. When no policy is enabled or loaded, the field is `null`.

## Consequences

Readiness diagnostics now show the active process policy version without changing data-plane transport behavior. Future signed policy revisions can replace this field with a typed revision ID once the policy distribution path owns immutable revisions.
