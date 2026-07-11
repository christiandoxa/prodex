# ADR 0694: File Secret Path Must Be Non-Empty

## Status

Accepted.

## Context

The file secret backend uses `SecretLocation::File` as its filesystem target.
An empty `PathBuf` is not a meaningful secret location, but the shared file
backend helper accepted it before attempting filesystem operations.

## Decision

`FileSecretBackend` rejects empty file secret paths before read, write, delete,
or revision probing proceeds.

## Consequences

Existing absolute and relative secret file paths remain valid. Empty file secret
locations now fail closed at the secret-store boundary instead of reaching
filesystem behavior for the current directory or another ambiguous target.
