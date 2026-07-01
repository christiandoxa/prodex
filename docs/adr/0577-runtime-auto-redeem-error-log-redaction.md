# ADR 0577: Runtime Auto-Redeem Error Log Redaction

## Status

Accepted

## Context

Runtime auto-redeem probes usage and consumes reset credits on the quota path.
Failures are logged for operators, but the raw error strings can include
headers, bearer tokens, key-bearing URLs, or upstream diagnostic material.

The auto-redeem logs already flatten newlines so each failure remains a single
runtime log record. They also need the same secret-like redaction used by other
runtime diagnostics.

## Decision

Normalize auto-redeem error log values through a small local helper that
redacts secret-like text and preserves single-line runtime log output.

## Consequences

- Auto-redeem availability, consume, and refresh failures keep operator-visible
  context without persisting bearer tokens or key assignments.
- Quota reservation, credit consumption, retry, and selection behavior are
  unchanged.
- The helper stays local to auto-redeem because it preserves that path's
  existing one-line log formatting.
