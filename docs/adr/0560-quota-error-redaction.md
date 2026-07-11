# ADR 0560: Redact quota diagnostics

## Status

Accepted

## Context

Quota commands and watch views collect provider failures into strings before
rendering terminal, JSON, and TUI output. Raw quota errors can include
authorization headers, key-bearing upstream URLs, provider routing details, or
backend response snippets.

## Decision

Quota support now redacts secret-like text at the quota error boundary before
errors are stored in `QuotaReport`, watch snapshots, virtual provider reports,
or external provider detail rows. Quota success data and status calculation are
unchanged.

## Consequences

- All quota renderers receive safer diagnostics by default.
- Bearer tokens and key-bearing URL query values are removed before display.
- Regression coverage pins the shared quota error helper.
