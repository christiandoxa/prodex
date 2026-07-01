# ADR 0456: Secret store display errors are redacted

## Status

Accepted.

## Context

Secret-store response planning already returns stable generic errors, but local
`Display` text included filesystem paths, backend locations, and backend error
reasons. Those values can reveal credential filenames, keyring accounts, or
operator environment details.

## Decision

`SecretError::Display` and `RefreshLeaseError::Display` now use generic messages
without paths, locations, backend names, or I/O reasons. Structured fields remain
available for trusted diagnostics.

## Consequences

Accidental display paths no longer expose credential storage locations or refresh
lease lock paths.
