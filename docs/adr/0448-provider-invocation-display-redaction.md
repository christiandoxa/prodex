# ADR 0448: Provider invocation display errors are redacted

## Status

Accepted.

## Context

Provider invocation response plans already return stable redacted authorization
failures, but the local `Display` text for credential-scope mismatches included
the rejected scope. That can expose control-plane or break-glass internals if a
composition root accidentally surfaces `Display`.

## Decision

`ProviderInvocationError::Display` now uses a generic credential-scope mismatch
message. Structured enum fields remain available for trusted diagnostics.

## Consequences

Provider invocation failures no longer expose credential-scope names through
generic display paths. Provider secret references remain represented by
`SecretRef` and stay redacted in debug output.
