# ADR 0759: Redact domain audit export-format error debug output

## Status

Accepted

## Context

`AuditExportFormat::parse` rejects raw client-provided export format values
before audit exports are planned. Rejected format strings can contain tenant
data, resource IDs, audit chain material, or credentials when callers pass
untrusted request input.

## Decision

Implement custom `Debug` for `AuditExportFormatError`. Keep only the `Empty`
and `Unknown` variant names in formatter output.

## Consequences

Diagnostics still distinguish empty export format input from unsupported
format input. Future export-format error variants must make redaction decisions
explicitly instead of inheriting derived `Debug`.
