# ADR 0287: Kubernetes Control-Plane Secret Scope

## Status

Accepted.

## Context

The Kubernetes baseline includes a placeholder `prodex-control-plane` deployment
scaled to zero until the async adapter exists. It previously mounted the full
gateway secret set, including provider API keys and gateway bearer tokens. The
enterprise architecture separates control-plane management from data-plane
inference, so the control-plane workload should not inherit data-plane provider
credentials by default.

## Decision

Add a dedicated `prodex-control-plane-secrets` ExternalSecret containing
PostgreSQL and Redis connection references, and make the control-plane
Deployment mount that secret instead of `prodex-gateway-secrets`.

The deployment security guard now requires the control-plane secret marker and
its self-test rejects manifests without it.

## Consequences

- The control-plane placeholder follows least-privilege secret scoping before it
  is scaled above zero.
- Provider API keys and gateway bearer tokens remain scoped to gateway
  data-plane pods.
- Future control-plane adapters can add specific secret references deliberately
  instead of inheriting all gateway runtime secrets.
