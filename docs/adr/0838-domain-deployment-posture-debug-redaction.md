# ADR 0838: Redact domain deployment posture debug output

Status: Accepted

## Context

`ContainerSecurityPolicy` and `KubernetesArtifactSet` carry detailed deployment
security posture. Derived `Debug` output exposed individual posture booleans
through diagnostics.

## Decision

Use custom `Debug` implementations for deployment posture inputs that report
only violation counts.

## Consequences

Callers still receive typed posture fields and typed violation lists, but
diagnostics no longer expose exact deployment hardening or Kubernetes artifact
gaps through posture debug output.
