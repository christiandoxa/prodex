# ADR 0459: Runtime metrics do not expose profile labels

## Status

Accepted.

## Context

ADR 0348 keeps profile names out of metric labels because they can identify
accounts and create high-cardinality series. Runtime broker metrics still
included the active profile in the `info` metric and per-profile inflight labels.

## Decision

Remove the active profile label from runtime broker Prometheus metadata. Render
profile inflight as a broker-level aggregate instead of one time series per raw
profile name. Structured snapshots still keep profile details for trusted
diagnostics.

## Consequences

Prometheus no longer exposes profile/account names or creates profile-specific
series. Operators still get total broker inflight pressure from the metric.
