# ADR 0566: Redact expose tunnel status errors

## Status

Superseded by [ADR 1068](1068-expose-session-tunnel-model.md)

## Context

`prodex expose` starts a local terminal relay and optionally opens a
`cloudflared` quick tunnel. When tunnel startup failed, the terminal status panel
rendered the detailed `{err:#}` chain directly. That chain can include
environment-specific command diagnostics, bearer tokens, or key-bearing URLs.

## Decision

Keep the existing tunnel-unavailable status and operator guidance, but pass the
detailed tunnel startup error chain through the shared secret-like text redactor
before rendering it in the terminal panel.

## Consequences

- `prodex expose` still tells operators when an explicitly requested tunnel is
  unavailable and continues with loopback-only access.
- Secret-like values from tunnel startup diagnostics are removed from the local
  terminal status.
- ADR 1068 replaces the legacy access-token and default-tunnel behavior.
