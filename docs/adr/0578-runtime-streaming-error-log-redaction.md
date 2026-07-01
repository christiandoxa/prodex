# ADR 0578: Runtime Streaming Error Log Redaction

## Status

Accepted

## Context

Runtime streaming response forwarding logs local writer failures and upstream
stream read failures. Those errors are useful for diagnosing transport
breakage, but `io::Error` text can carry copied headers, bearer tokens,
key-bearing URLs, or provider diagnostic material.

The streaming path already keeps client responses transport-transparent. The
remaining boundary was runtime log persistence.

## Decision

Normalize streaming error log values through a local helper that redacts
secret-like text and preserves single-line log records.

## Consequences

- Streaming writer and read failure logs keep request, profile, byte, chunk,
  and elapsed-time context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Streaming semantics, cancellation behavior, and transport failure
  classification are unchanged.
