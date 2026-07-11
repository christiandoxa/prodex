# ADR 1068: Safe expose session and tunnel model

## Status

Accepted

## Context

`prodex expose` previously launched a Cloudflare quick tunnel by default and
placed one long-lived bearer capability in both the URL path and query string.
The same capability appeared separately in the status panel. Authorized HTTP
requests spawned unbounded threads, browser input sent one request per
keypress, and a missing `Origin` header was accepted for terminal mutation.

The exposed process is an interactive shell. Accidental publication or token
disclosure therefore grants remote code execution with the operator's account.

## Decision

- The HTTP listener remains fixed to `127.0.0.1`. Public access requires the
  explicit `--tunnel` flag and renders a remote-shell warning.
- `--no-tunnel` remains as a hidden deprecated no-op compatibility alias until
  version `0.300.0`. It emits a warning and conflicts with `--tunnel`.
- The operator receives a two-minute, one-time bootstrap capability in the URL
  fragment. Fragments are not sent in HTTP request targets. Browser code clears
  the fragment before exchanging the capability in an `Authorization` header.
- A successful exchange issues an opaque 15-minute session cookie with
  `HttpOnly`, `SameSite=Strict`, `Path=/expose`, and `Secure` for HTTPS origins.
  Sessions expire after ten minutes idle, rotate every five minutes while the
  page is active, and can be revoked explicitly when the page closes.
- Browser mutations require an exact allowed `Host`, an exact same-origin
  `Origin`, the session cookie, and a session-bound CSRF header. Missing or
  duplicate security headers fail closed. Tunnel hosts enter the allowlist only
  after `cloudflared` reports the exact quick-tunnel hostname.
- The relay uses a loopback `TcpListener` and a fixed worker pool: at most 32
  stream workers plus four short request workers. It does not use the
  connection-spawning `tiny_http` server. Its accepted-connection queue and
  each client output queue hold at most 64 items. Slow request reads and writes
  have a five-second timeout; request headers, header count, target length, and
  body size are bounded. Scrollback remains capped at 1 MiB.
- Browser input is batched for 18 ms and capped at 16 KiB per request. Each
  session is limited to 64 input requests and 64 KiB per second.
- Output continues through SSE to reuse the existing bounded scrollback and
  output queues. Input and session endpoints enforce body limits and return
  explicit overload responses.
- Shutdown terminates the PTY child and tunnel process, closes the relay, and
  joins accept, worker, PTY reader/waiter, and tunnel reader threads.
- Session and CSRF capabilities are never rendered. Bootstrap, session, and
  CSRF capabilities are never logged or included in request paths, query
  strings, error text, debug output, telemetry, or snapshots. The sole delivery
  exception is the complete one-time bootstrap URL or URLs shown in the local
  operator's `Expose` panel: the same bootstrap appears only as a fragment in
  the local URL and, when explicitly requested, its tunnel URL, never as a
  separate field. This exception is necessary for the operator to open or
  deliberately share the session, expires after two minutes, and becomes
  invalid after one exchange.

## Consequences

- Existing `prodex expose` invocations become loopback-only. Operators who need
  remote access must add `--tunnel` and acknowledge the warning.
- Old bookmarked token URLs stop working. Each invocation produces a new
  short-lived one-time URL.
- Reloading after the fragment is cleared requires reopening the current
  one-time URL or starting a new expose session.
- Fixed workers can reject bursts with `503`, and client/session limits can
  return `429`; work no longer grows without a bound.
- The deprecated `--no-tunnel` alias can be deleted at `0.300.0` after release
  notes have covered the default change.
