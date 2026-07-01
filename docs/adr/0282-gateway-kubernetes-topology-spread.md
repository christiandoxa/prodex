# ADR 0282: Gateway Kubernetes Topology Spread

## Status

Accepted.

## Context

The Kubernetes gateway baseline runs three gateway replicas with a
PodDisruptionBudget and HPA. Without scheduler placement constraints, those
replicas can still land on a single node or zone. That weakens the enterprise
high-availability target even when the deployment has multiple replicas.

## Decision

Add gateway `topologySpreadConstraints` to the Kubernetes manifest:

- spread across `topology.kubernetes.io/zone` with `ScheduleAnyway` so clusters
  without zone labels can still run the baseline; and
- spread across `kubernetes.io/hostname` with `DoNotSchedule` so replicas do not
  concentrate on one node when alternatives exist.

The deployment security guard now requires those markers and its self-test
rejects a manifest without gateway topology spreading.

## Consequences

- The production-shaped baseline has concrete failure-domain evidence beyond
  replica count, PDB, and HPA.
- Small local clusters can still schedule the gateway when zone labels are not
  available.
- Operators may tune topology policies for their cluster, but removing them must
  be deliberate and will fail the repository guard.
