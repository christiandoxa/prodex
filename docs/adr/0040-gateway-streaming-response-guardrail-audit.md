# ADR 0040: Gateway streaming response guardrail denials are audited

## Status

Accepted

## Context

Buffered response guardrail denials already emit immutable audit events with metadata-only details. Streaming response guardrails previously stopped the stream and wrote a runtime diagnostic log, but did not append an audit event.

In regulated deployments, output policy enforcement is security-sensitive regardless of whether the upstream response is buffered or streamed. A streaming-only audit gap would make incident reconstruction and policy compliance evidence incomplete.

## Decision

When the streaming output guardrail detects a blocked output keyword, the stream reader appends a `gateway_data_plane` audit event:

- `action`: `response_guardrail_blocked`
- `outcome`: `failure`
- `details.reason`: `blocked_output_keyword`
- `state_backend`: the active gateway state backend label

The audit payload deliberately omits the matched output value, gateway bearer token, prompt, and response body. Runtime diagnostics continue to record only metadata and `matched_value_redacted=true`.

## Consequences

Security-sensitive output enforcement now has audit coverage for both buffered and streaming responses. Because response headers may already have been committed for streams, the stream path still preserves existing streaming semantics by ending the body rather than attempting to replace the response with a new JSON error envelope.

## Validation

A regression test exercises an end-to-end streaming response whose output contains a blocked marker. The test verifies that:

- the stream response is truncated without exposing the blocked marker;
- an immutable audit event is written with `response_guardrail_blocked` and `blocked_output_keyword`;
- neither the audit log nor runtime log contains the gateway token or matched output marker;
- the runtime log records `gateway_guardrail_stream_blocked` with redaction metadata.
