# ADR 0331: Health Probe Active Policy Revision Metadata

## Status

Accepted.

## Context

Gateway readiness must expose the active policy/configuration revision so
operators can tell which revision a replica is serving. `HealthSnapshot` already
stores `active_policy_revision` and readiness fails when it is absent, but the
public `HealthProbeResponsePlan` did not carry that revision as typed metadata.

## Decision

Add `active_policy_revision: Option<PolicyRevisionId>` to
`HealthProbeResponsePlan` and populate it from the snapshot in
`plan_health_probe_response`.

The human-readable health message remains stable and redacted; adapters can
serialize the typed field for `/readyz`, `/livez`, and `/startupz` without
parsing messages or reading configuration state directly.

## Consequences

- Readiness adapters can expose the active revision from the domain health
  contract.
- Missing active revision still fails readiness.
- Revision IDs are machine-readable metadata and are not leaked through
  free-form health messages.
