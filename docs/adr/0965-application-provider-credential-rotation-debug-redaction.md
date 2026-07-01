# ADR 0965: Application Provider Credential Rotation Debug Redaction

## Status

Accepted

## Context

Application provider-credential rotation plans carry control-plane action
details, provider credential reference metadata, audit digest state,
authorized/denied decisions, and nested storage planning. Derived debug output
would expose tenant identifiers, provider names, and provider credential paths
in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationProviderCredentialRotationRequest`,
`ApplicationProviderCredentialRotationPlan`, and
`ApplicationProviderCredentialRotationError`. Redact control-plane action
details, provider credential reference metadata, audit digests, decisions,
storage plans, route/resource mismatches, tenant mismatches, and nested
storage errors while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish provider-credential rotation planner
shapes without leaking tenant identifiers, provider names, provider credential
paths, or audit digest material.
