# 0180: Domain Audit Outcome Stable Validation Responses

## Status

Accepted

## Context

Audit outcomes are part of public audit query filters, exports, and
machine-readable event envelopes. The enum already serializes to a stable shape,
but gateway and control-plane adapters still need a domain-owned parser for
dynamic input. Raw unknown values may contain copied headers, provider payload
fragments, or credentials and must not be echoed in client-visible errors.

## Decision

`prodex-domain` owns `AuditOutcome::parse`,
`AuditOutcome::as_str`, and `plan_audit_outcome_error_response`.

Accepted outcome strings are exactly `success`, `denied`, and `failed`.
Validation failures map to stable status/code/message response plans that do not
echo rejected outcome strings, tenant identifiers, principal identifiers,
resource identifiers, audit chain material, or credential-like material.

## Consequences

- Gateway and control-plane adapters can validate audit outcome filters and
  mutation inputs without duplicating enum parsing.
- Existing serialized audit outcomes remain unchanged.
- Raw invalid outcome values remain available only to trusted diagnostics.
