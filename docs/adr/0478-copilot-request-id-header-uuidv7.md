# ADR 0478: Copilot request ID headers use UUIDv7

## Status

Accepted.

## Context

The Copilot bridge set `x-request-id` to `prodex-{request_id}`. Runtime request
IDs are local proxy sequence values and can collide across multi-replica
deployments.

## Decision

Generate the Copilot upstream `x-request-id` value with the typed domain
`RequestId` UUIDv7 generator while preserving the existing `prodex-` prefix.

## Consequences

Copilot upstream request correlation no longer depends on a process-local
request sequence.
