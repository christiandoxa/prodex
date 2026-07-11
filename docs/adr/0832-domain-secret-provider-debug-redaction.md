# ADR 0832: Redact domain secret provider debug names

Status: Accepted

## Context

`SecretProviderDescriptor` carries provider names used to route secret
resolution. Derived `Debug` output exposed those names through diagnostics.

## Decision

Use a custom `Debug` implementation for `SecretProviderDescriptor` that
preserves provider kind and rotation capability while redacting provider name.

## Consequences

Diagnostics can still distinguish provider class and rotation support, but
secret provider names no longer appear through descriptor debug output.
