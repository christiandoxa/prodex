# ADR 0574: Redact profile probe refresh errors

## Status

Accepted

## Context

Runtime profile probe refresh logs record quota probe failures and state update
failures for both inline startup probes and queued background probes. These
errors previously used raw provider or `{err:#}` text in runtime logs. Probe
errors can include bearer tokens, key-bearing URLs, proxy diagnostics, or
provider response details.

## Decision

Pass profile probe refresh error text and state-update error chains through the
shared secret-like text redactor before logging them.

## Consequences

- Runtime logs still include probe event names, profile names, lag, and
  `state_update:` classification.
- Secret-like provider/proxy/state-update diagnostics are removed from probe
  refresh error fields.
- Probe refresh scheduling, apply timeout, and profile health behavior are
  unchanged.
