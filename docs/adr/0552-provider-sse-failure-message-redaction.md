# ADR 0552: Redact provider SSE failure messages

## Status

Accepted

## Context

Gemini and DeepSeek compatibility streams emit local `response.failed` SSE
events when upstream stream parsing or provider compatibility translation fails.
Those messages can include transport or parser diagnostics. If the diagnostic
contains an auth header, provider URL, or key-bearing query string, returning it
verbatim violates the gateway error redaction boundary.

## Decision

`runtime_provider_sse_failed_event` redacts secret-like text from the failure
message before serializing the SSE event. The event type, sequence number,
response ID, and error code are unchanged.

## Consequences

- Existing provider stream failure semantics stay compatible.
- Bearer tokens and key-bearing endpoint query values are removed from
  client-visible provider SSE failure messages.
- Regression coverage pins the shared helper so Gemini and DeepSeek callers do
  not need per-provider guards.
