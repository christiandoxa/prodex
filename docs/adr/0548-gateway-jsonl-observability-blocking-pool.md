# ADR 0548: Write gateway JSONL observability on the blocking pool

## Status

Accepted

## Context

The legacy local rewrite gateway can export gateway spend events to a JSONL
file. That sink created directories, opened the file, serialized JSON, and
appended the line inline while emitting the request spend event.

## Decision

Keep the existing JSONL format, but move the filesystem write into the runtime
blocking pool. The request path still logs the normal runtime spend event and
schedules ledger reconciliation as before.

## Consequences

Slow filesystems no longer block the spend-event emitter. JSONL failures remain
redacted runtime log events with the request sequence, configured path, and I/O
error.
