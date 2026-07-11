# ADR 0041: Gateway guardrail webhook denials are audited

## Status

Accepted

## Context

Guardrail webhooks are external policy enforcement points. A webhook may deny a request before upstream routing (`pre`) or deny a buffered response after upstream completion (`post`). These denials returned policy errors and wrote runtime diagnostics, but did not consistently emit immutable audit events.

In regulated environments, external policy denials must be reconstructable from audit logs without exposing gateway credentials, webhook bearer tokens, request bodies, response bodies, or webhook messages.

## Decision

Guardrail webhook denials now append metadata-only data-plane audit events:

- pre-request denials use `action=guardrail_webhook_blocked`;
- post-response denials use `action=response_guardrail_webhook_blocked`;
- both include `state_backend`, `phase`, and sanitized `reason` metadata;
- pre-request denials also include the request path.

Audit events and runtime diagnostics deliberately omit matched values, prompt/response bodies, gateway bearer tokens, webhook bearer tokens, and webhook message text. Runtime logs retain `matched_value_redacted=true` for operator visibility.

## Consequences

Policy enforcement through webhooks has immutable audit coverage consistent with local request, buffered response, and streaming response guardrails. Operators can identify which phase and reason blocked traffic without leaking sensitive payloads into shared logs.

## Validation

Regression tests cover both pre-request and post-response webhook denials. They verify that:

- the gateway returns the expected policy error;
- the audit action, phase, reason, and path metadata are present where applicable;
- audit and runtime logs do not contain gateway tokens, webhook bearer tokens, or webhook message text;
- runtime diagnostics record the webhook block with redaction metadata.
