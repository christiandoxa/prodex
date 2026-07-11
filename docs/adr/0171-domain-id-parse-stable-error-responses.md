# 0171: Domain ID Parse Stable Error Responses

## Status

Accepted

## Context

Enterprise APIs use typed UUIDv7 identifiers for tenants, principals, requests,
calls, reservations, virtual keys, policy revisions, and audit events. Parsing
failures can include raw client-provided identifier values. Those values are
high-cardinality and may contain pasted credentials, path fragments, tenant
names, or other sensitive strings.

Raw parse errors remain useful for trusted diagnostics, but API boundaries need
stable client-visible responses that do not echo invalid identifier inputs.

## Decision

`prodex-domain` owns `plan_id_parse_error_response`.

The planner maps `IdParseError` to a stable status/code/message response plan.
It preserves the identifier kind in the machine-readable code, while
deliberately omitting the raw submitted value, UUID parser details, tenant
names, credential-like fragments, and high-cardinality IDs.

## Consequences

- Gateway and control-plane adapters can reject malformed typed IDs without
  leaking raw path/query/body values.
- Identifier kinds remain machine-readable for clients and tests.
- Raw parse errors stay available only to trusted diagnostics.
