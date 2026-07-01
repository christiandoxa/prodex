# ADR 0964: Application Provider Credential Reference Debug Redaction

## Status

Accepted

## Context

Application provider-credential reference plans carry tenant-scoped provider
credential commands and nested storage plans. Derived debug output would expose
tenant identifiers, provider names, and provider credential storage paths in
diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationProviderCredentialReferenceRequest`,
`ApplicationProviderCredentialReferencePlan`, and
`ApplicationProviderCredentialReferenceError`. Redact provider credential
commands, storage plans, and nested storage errors while preserving planner and
error variant names.

## Consequences

Application diagnostics can distinguish provider-credential reference planner
shapes without leaking tenant identifiers, provider names, or provider
credential storage paths.
