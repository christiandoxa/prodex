# ADR 0021: Audit Gateway Admin Authentication Failures

## Status

Accepted

## Context

Enterprise and regulated deployments need control-plane audit trails for both successful mutations and denied access attempts. The gateway already wrote admin key and SCIM mutations to the audit log, while admin authentication failures were only recorded in the runtime log. Runtime logs are operational diagnostics, not durable security audit evidence.

## Decision

Gateway admin authentication failures now append sanitized audit events with:

- `component: gateway_admin`
- `action: auth_failed`
- `outcome: failure`
- non-secret details: failure reason, HTTP method, path, and state backend label

Bearer tokens, generated virtual keys, and provider secrets are never included in the audit payload.

## Consequences

Operators can correlate denied control-plane access attempts through the same audit log used for admin mutations. This does not alter upstream data-plane pass-through behavior or expose credential material.
