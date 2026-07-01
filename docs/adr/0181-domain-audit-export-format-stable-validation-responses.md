# 0181: Domain Audit Export Format Stable Validation Responses

## Status

Accepted

## Context

Control-plane audit export endpoints need stable, machine-readable format
selection. Without a domain-owned parser, adapters can accept arbitrary format
strings from query parameters or request bodies. Unknown values may contain
copied headers, route fragments, provider payloads, or credentials and must not
be echoed in client-visible errors.

## Decision

`prodex-domain` owns `AuditExportFormat`,
`AuditExportFormat::parse`, and
`plan_audit_export_format_error_response`.

Accepted export formats are exactly `jsonl` and `csv`. The domain type also
provides the stable content type and file extension for each format. Validation
failures map to stable status/code/message response plans that do not echo
rejected format strings, tenant identifiers, principal identifiers, resource
identifiers, audit chain material, or credential-like material.

## Consequences

- Gateway and control-plane adapters can validate audit export format inputs
  without duplicating parser behavior.
- Export response metadata can be planned from a domain type.
- Raw invalid export format values remain available only to trusted
  diagnostics.
