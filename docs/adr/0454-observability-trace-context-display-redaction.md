# ADR 0454: Trace context display errors are redacted

## Status

Accepted.

## Context

Trace-context response planning already returns a stable redacted error, but
local `Display` text named specific headers and fields. That can leak parsing
details if a composition root accidentally surfaces the display string.

## Decision

`TraceContextError::Display` now uses a generic invalid trace-context message
for local header and field validation failures. Structured enum variants remain
available for tests and trusted diagnostics.

## Consequences

Accidental display paths no longer expose trace header names or field names,
while response envelopes remain unchanged.
