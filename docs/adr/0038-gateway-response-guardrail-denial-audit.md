# ADR 0038: Audit buffered gateway response guardrail denials without logging matched output

## Status

Accepted

## Context

Gateway guardrails support response-side blocked output keywords. Buffered successful
upstream responses are inspected before being returned to the client, and a matching
blocked output keyword causes the gateway to return `403 policy_violation` instead of the
provider response.

Response guardrail denials are security-sensitive data-plane decisions. They may involve
provider output content and credentialed requests, so audit records must not persist bearer
tokens or matched output values.

## Decision

Buffered response guardrail denials now append a `gateway_data_plane` audit event with
action `response_guardrail_blocked`, outcome `failure`, backend label, and non-secret
reason `blocked_output_keyword`. The audit record deliberately omits Authorization headers,
bearer tokens, matched keywords, and response snippets.

## Consequences

Operators can investigate response-side guardrail denials from the immutable audit trail
without storing potentially sensitive provider output. Existing HTTP behavior remains
unchanged for buffered responses: blocked output still returns `403 policy_violation`
before reaching the client.
