# ADR 0572: Redact scheduled save error logs

## Status

Accepted

## Context

Runtime scheduled-save workers persist state snapshots, continuation journals,
and smart-context artifacts in the background. Write failures were recorded in
runtime logs with detailed `{err:#}` chains. Those chains can include local
paths, environment-specific diagnostics, bearer tokens, or key-bearing URLs from
lower-level storage helpers.

## Decision

Route scheduled-save error chains through the shared secret-like text redactor
before placing them in structured runtime log `error` fields.

## Consequences

- Runtime logs still include save failure event names, stage, reason, lag, and
  revision metadata.
- Secret-like material is removed from scheduled-save error details.
- State, continuation-journal, and artifact save behavior is unchanged.
