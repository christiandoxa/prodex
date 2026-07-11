# ADR 0603: Domain Telemetry Secret Prefix Metric Label Value Guard

## Status

Accepted.

## Context

The domain telemetry boundary rejects identifier-like and secret-like metric
label keys, plus UUID-shaped values. A token could still appear as the value of
an otherwise allowed categorical key such as `provider`.

## Decision

Metric label value validation now rejects common credential prefixes:
`Bearer `, `sk-`, `ghp_`, `gho_`, and `github_pat_`.

## Consequences

Credential material must remain outside metrics. Closed categorical values such
as provider, result, and status labels remain valid.
