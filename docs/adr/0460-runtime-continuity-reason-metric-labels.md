# ADR 0460: Runtime continuity reason metrics use closed labels

## Status

Accepted.

## Context

Runtime continuity failure counters are built from runtime log `reason` fields.
Those values are useful in trusted diagnostics, but Prometheus labels must not
carry raw log text, tenant/account hints, request identifiers, or credential-like
material.

## Decision

The Prometheus renderer maps known continuity reasons to a closed allowlist and
groups unknown reasons under `other`. Structured snapshots still retain raw
reason keys for trusted diagnostics.

## Consequences

Continuity metrics remain useful without exposing arbitrary runtime-log reason
values or creating unbounded label cardinality.
