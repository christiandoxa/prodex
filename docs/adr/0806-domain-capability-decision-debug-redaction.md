# ADR 0806: Redact capability decision debug output

Status: Accepted

## Context

`CapabilityDecision` can carry selected route candidates and missing capability
sets. Its derived `Debug` formatter exposed provider/model topology and missing
capability internals through diagnostics.

## Decision

Use a custom `Debug` implementation for `CapabilityDecision` that preserves the
decision shape while redacting candidate and missing-capability details.

## Consequences

Diagnostics can still distinguish compatible, incompatible, and no-candidate
outcomes, but raw routing topology and missing capability internals no longer
appear through `CapabilityDecision` debug output.
