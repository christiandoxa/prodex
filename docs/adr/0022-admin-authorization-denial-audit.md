# ADR 0022: Audit Gateway Admin Authorization Denials

## Status

Accepted

## Context

Enterprise control planes need evidence not only for successful mutations and authentication failures, but also for authorization decisions that deny access to a resource. Gateway admin key and SCIM handlers enforce tenant/team/project/user/key-prefix scope checks, but scope denials were previously only surfaced to the caller as `403` responses.

## Decision

Gateway admin scope denials now append sanitized audit events with:

- `component: gateway_admin`
- `action: authorization_denied`
- `outcome: failure`
- details containing non-secret actor metadata, denied resource type, operation, resource name, state backend, and `reason: scope_forbidden`

The denied response body and status are unchanged. Tokens, generated virtual-key secrets, upstream credentials, and provider secrets are not logged.

## Consequences

Operators can review denied control-plane authorization attempts through the same audit log as admin mutations and authentication failures. The change is limited to Prodex-owned control-plane handlers and does not affect upstream data-plane pass-through behavior.
