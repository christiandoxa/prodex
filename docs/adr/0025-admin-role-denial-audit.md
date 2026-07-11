# ADR 0025: Audit Gateway Admin Role Denials

## Status

Accepted

## Context

The gateway admin router enforces vertical authorization by rejecting mutation attempts from viewer-like principals. Those role denials were returned to callers and written to the runtime log, but they were not captured in the durable security audit log. Enterprise environments need audit evidence for failed privilege-escalation attempts as well as authentication failures and scope denials.

## Decision

Gateway admin role denials now append sanitized audit events with:

- `component: gateway_admin`
- `action: authorization_denied`
- `outcome: failure`
- details containing `reason: role_forbidden`, actor name, role, method, path, and state backend label

No bearer token, generated virtual key secret, provider credential, or request body is written to the audit payload.

## Consequences

Operators can review vertical privilege-denial attempts through the same audit log used for admin mutations, authentication failures, and scope denials. The existing `403 gateway_admin_role_forbidden` API response remains unchanged.
