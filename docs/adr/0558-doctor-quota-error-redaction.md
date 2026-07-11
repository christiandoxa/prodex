# ADR 0558: Redact doctor quota errors

## Status

Accepted

## Context

`prodex doctor --quota` renders per-profile quota failures in the terminal
panel. The panel previously took the first line of the raw provider error. That
line can include authorization headers, key-bearing URLs, or provider routing
details.

## Decision

Doctor quota errors now pass through the shared secret-like text redactor before
the first-line summary is rendered. Quota success rendering and status coloring
are unchanged.

## Consequences

- Operators still see that quota probing failed.
- Bearer tokens and key-bearing URL query values are removed from doctor quota
  summaries.
- Regression coverage pins the doctor quota summary boundary.
