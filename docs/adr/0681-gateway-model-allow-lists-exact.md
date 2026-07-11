# ADR 0681: Gateway Model Allow Lists Match Exactly

## Status

Accepted.

## Context

Gateway virtual keys and guardrails can restrict requests by model ID. These
allow lists are authorization and policy inputs. Policy validation only rejected
trim-empty values, and runtime config resolution trimmed configured model IDs
before putting them into active state.

## Decision

Configured gateway model allow-list entries must now be exact non-empty values
without whitespace. Runtime config resolution no longer trims model allow-list
entries; if policy settings are built directly, whitespace-bearing model IDs
are ignored rather than normalized.

## Consequences

Canonical model IDs are unchanged. Padded or whitespace-bearing model IDs fail
closed in policy files and no longer become active virtual-key or guardrail
model policy through trim-normalization.
