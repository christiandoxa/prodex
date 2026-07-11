# ADR 0585: Runtime Background Worker Error Log Redaction

## Status

Accepted

## Context

Runtime background worker startup and panic failures are written to runtime logs.
Those values may include copied headers, bearer tokens, key-bearing URLs, local
paths, or worker panic payloads with diagnostic material.

Worker spawning and panic propagation must remain unchanged. The boundary to
harden is the persisted runtime log field.

## Decision

Route background worker spawn and panic error log values through a local helper
that redacts secret-like text and keeps structured log fields single-line.

## Consequences

- Worker failure logs keep worker name and failure context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Background worker startup, panic logging, and panic rethrow behavior are
  unchanged.
