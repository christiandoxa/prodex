# ADR 0037: Enforce and audit gateway request guardrail denials before upstream transport

## Status

Accepted

## Context

Gateway guardrail configuration supports request-side blocked keywords, output blocked
keywords, allowed models, prompt-injection detection, and PII redaction. The data-plane
request path already applied PII redaction and webhook guardrail checks, but local
request-side blocked keyword checks were not enforced in the app path before upstream
transport.

Request guardrail denials are security-sensitive data-plane decisions. They may include
user-controlled prompt text and credentialed requests, so audit records must not persist
bearer tokens or matched prompt content.

## Decision

The gateway data-plane request path now evaluates local request guardrails before PII
redaction, virtual-key admission, webhook checks, and upstream transport. A blocked request
returns the guardrail kind as the existing JSON error code shape, and appends a
`gateway_data_plane` audit event with action `guardrail_blocked`, outcome `failure`,
request path, backend label, and non-secret reason such as `blocked_keyword`.

Audit records deliberately omit Authorization headers, bearer tokens, and matched prompt
values.

## Consequences

Configured request-side blocked keywords now reliably stop traffic before any upstream
provider call. Operators can investigate guardrail denials from the immutable audit trail
without storing sensitive prompt snippets or tokens. This is a security-tightening change
that preserves the gateway JSON error envelope while adding enforcement for an already
configured policy surface.
