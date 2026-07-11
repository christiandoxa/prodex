# ADR 0695: Runtime Proxy Preset Rejects Whitespace Padding

## Status

Accepted.

## Context

`runtime_proxy.preset` selects a named local admission/concurrency preset.
The parser accepted leading and trailing whitespace before matching the preset
name, so malformed policy input could normalize into active runtime tuning.

## Decision

Runtime proxy preset parsing rejects values with leading or trailing whitespace.
The parser remains case-insensitive for canonical preset names to preserve
existing non-security-sensitive compatibility.

## Consequences

Canonical preset values such as `default` and `many-terminals` remain valid.
Padded preset values now fail policy loading instead of silently activating
runtime tuning.
