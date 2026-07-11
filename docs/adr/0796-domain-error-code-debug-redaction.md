# ADR 0796: Redact error code debug output

Status: Accepted

## Context

`ErrorCode` carries stable machine-readable envelope codes. Its derived `Debug`
formatter exposed raw code strings to diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `ErrorCode` that preserves code presence
while redacting the raw value.

## Consequences

Serialization and explicit `as_str()` access still expose public error codes
where intended, but raw code values no longer appear through `ErrorCode` debug
output.
