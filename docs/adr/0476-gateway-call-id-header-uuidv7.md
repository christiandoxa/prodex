# ADR 0476: Gateway call ID headers use UUIDv7

## Status

Accepted.

## Context

The optional gateway call ID response header used `prodex-{request_id}`. Runtime
request IDs are local proxy sequence values and can collide across multi-replica
deployments.

## Decision

Generate gateway call ID response header values with the typed domain `CallId`
UUIDv7 generator while preserving the configured header name and existing
`prodex-` value prefix.

## Consequences

Gateway response call IDs no longer expose process-local request sequences.
Operators can still correlate local runtime logs through the separate runtime
request ID.
