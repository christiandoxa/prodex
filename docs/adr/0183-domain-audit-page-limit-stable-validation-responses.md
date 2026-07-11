# 0183: Domain Audit Page Limit Stable Validation Responses

## Status

Accepted

## Context

Audit query and export endpoints must bound result sizes to protect control
plane latency, storage load, and export memory usage. If each adapter validates
raw page-size integers independently, deployments can drift on defaults,
maximums, and client-visible error shapes.

## Decision

`prodex-domain` owns `AuditPageLimit` and
`plan_audit_page_limit_error_response`.

`AuditPageLimit::new` defaults omitted limits to `100`, rejects zero, and caps
accepted limits at `1_000`. Validation failures map to stable
status/code/message response plans that do not echo rejected values, tenant
identifiers, principal identifiers, resource identifiers, audit chain material,
or credential-like material.

## Consequences

- Gateway and control-plane adapters can share one audit pagination limit
  contract.
- Storage adapters receive already bounded query/export limits.
- Raw invalid limit values remain available only to trusted diagnostics.
