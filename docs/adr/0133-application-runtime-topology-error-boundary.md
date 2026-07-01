# ADR 0133: Application runtime topology error boundary

## Status

Accepted

## Context

`prodex-application` validates runtime topology before gateway/control-plane
adapters start. Raw runtime planning errors can reveal migration placement,
replica counts, PostgreSQL/SQLite planning details, Redis coordination
requirements, or other deployment topology internals. Those details are useful
for trusted deployment diagnostics but should not become generic client-visible
or readiness response text.

## Decision

Add `plan_application_runtime_error_response` to `prodex-application`. It maps
migration, storage topology, and multi-replica accounting validation failures to
`runtime_topology_invalid`, and maps durable storage planning failures to
`runtime_storage_plan_unavailable`.

The response plan is HTTP-neutral and returns only status, code, and a generic
message. Raw errors remain available for trusted deployment diagnostics and
operator logs.

## Consequences

Startup/readiness adapters can report runtime topology failures consistently
without leaking request-path migration policy, replica counts, SQL backend
internals, Redis topology requirements, or DDL planning details.
