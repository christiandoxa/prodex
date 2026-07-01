# ADR 0501: Reject literal secret environment values in Kubernetes artifacts

## Status

Accepted

## Context

Production Kubernetes manifests should source data-plane, control-plane, and
provider credentials from ExternalSecret-backed Kubernetes Secrets. A future
edit could accidentally add a literal `value:` for token, password, API key, or
secret-bearing URL environment variables.

## Decision

The deployment security guard rejects Kubernetes artifacts that define
secret-bearing `PRODEX_*`, provider, or GitHub credential environment variables
with literal `value:` entries. They must stay behind ExternalSecret-backed
secret references.

## Consequences

CI catches static Kubernetes credential regressions before deployment review.
