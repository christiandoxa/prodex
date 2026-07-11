# ADR 0055: Redact Gemini Live local and provider stream errors

## Status

Accepted

## Context

Gemini Live support bridges a local websocket sidecar to the upstream Gemini Live
websocket API. Several local setup and stream-failure paths returned raw error
strings either as HTTP response bodies or as JSON websocket error frames. Those
raw strings can include auth preparation details, websocket handshake details,
provider endpoint names, or parser/transport diagnostics that should not be part
of the gateway client contract.

## Decision

Gemini Live client-visible failures now use stable redacted messages:

- `Gemini Live authentication could not be prepared`
- `Gemini Live websocket upgrade could not be prepared`
- `Gemini Live provider stream failed`

Runtime logs may still include detailed diagnostics for operators, but Gemini
Live error fields pass through secret-like text redaction before logging so
bearer tokens and key-bearing endpoint query values are not retained. Successful
Gemini Live streaming, local sidecar accept logging, and upstream transport
behavior are otherwise unchanged.

## Consequences

- Clients no longer receive local auth, endpoint, websocket, or provider stream
  diagnostic details from Gemini Live failures.
- Regression coverage pins the stable messages and rejects common credential,
  endpoint, and websocket-detail substrings.
- This preserves the secure-by-default API error posture while keeping redacted
  operator diagnostics in runtime logs.
