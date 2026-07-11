# ADR 0474: Gateway spend call IDs use UUIDv7

## Status

Accepted.

## Context

Runtime gateway spend events used `prodex-{request_id}` as `call_id`. The
runtime request ID is a local proxy sequence value, so it can collide across
multi-replica gateway deployments and is not a valid enterprise `CallId`
boundary.

## Decision

Generate gateway spend event call IDs with the typed domain `CallId` UUIDv7
generator while preserving the existing `prodex-` log prefix.

## Consequences

Gateway spend logs no longer expose a process-local request sequence as a call
identifier. The separate `request` field remains available for local runtime log
correlation.
