# ADR 0457: Session resolve display errors are redacted

## Status

Accepted.

## Context

ADR 0149 added stable redacted response planning for session resolution errors,
but `SessionResolveError::Display` still included the user-supplied selector and
ambiguous matching session IDs. Those values can reveal session identifiers,
prefixes, and affinity metadata when errors are formatted outside the explicit
response planner.

## Decision

`SessionResolveError::Display` now emits only stable generic messages for missing
and ambiguous selectors. The enum fields still retain selector and match details
for trusted diagnostics that deliberately inspect structured data.

## Consequences

Accidental display formatting no longer leaks session selectors or matching
session IDs into generic responses, logs, or operator surfaces.
