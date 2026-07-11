# ADR 0065: Kubernetes artifact baseline

## Status

Accepted.

## Context

The enterprise target requires production deployment artifacts: hardened
container settings, Kubernetes Deployment, Service, non-secret ConfigMap,
ExternalSecret references, PodDisruptionBudget, HorizontalPodAutoscaler,
NetworkPolicy, observability integration, and a migration Job. The repository
had a Dockerfile and compose scaffold but no Kubernetes manifest baseline.

## Decision

Add `deploy/kubernetes/prodex-gateway.yaml` as a production-shaped baseline. The
manifest uses an immutable image digest reference, non-root execution,
read-only root filesystems, dropped Linux capabilities, explicit resource
requests/limits, operational probes, ExternalSecret-backed secrets, PDB, HPA,
NetworkPolicy, and ServiceMonitor. Include a migration Job as a safe-failing
placeholder that documents the current gap: versioned migrator commands are not
yet implemented and request-serving pods must not become schema migrators.

## Consequences

- Operators have a concrete Kubernetes artifact to review and adapt.
- CI can now guard the presence of required deployment object kinds and security
  controls.
- The migration Job intentionally fails until Phase 2 introduces versioned
  migration commands; this prevents silently implying that gateway startup is a
  valid migration mechanism.
