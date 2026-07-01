# ADR 0566: Redact expose tunnel status errors

## Status

Accepted

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

- `prodex expose` still tells operators that the tunnel is unavailable and how
  to continue with `--no-tunnel`.
- Secret-like values from tunnel startup diagnostics are removed from the local
  terminal status.
- Local relay, access-token, CSP, and Cloudflare URL behavior are unchanged.
