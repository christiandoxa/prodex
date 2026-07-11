# ADR 1060: Development Environment And Rooted-File Secret Provider

## Status

Accepted.

## Context

The domain `SecretProvider` contract must also support local development without
adding a network client or making development credential sources part of the
production gateway boundary. Existing secret-store code already provides the
canonicalized, private, bounded projected-file reader needed for local files.

## Decision

Add `DevelopmentSecretProvider` to `prodex-secret-store`. It matches its
configured provider name exactly and accepts only these reference names:

- `env:NAME`, where `NAME` is a non-empty ASCII environment identifier.
- `file:relative/path`, resolved below the canonical configured root.

Environment reads are process-local and bounded. File reads reuse the projected
reader, which rejects traversal and escapes, requires regular private files,
checks the opened file identity, and enforces the 64 KiB development limit. The
adapter performs no network or subprocess I/O, returns only redacted domain
resolution errors, and does not claim versioned rotation support.

## Consequences

- Development composition roots can use environment variables or checked local
  files through the same typed domain reference contract.
- Absolute paths, traversal, symlink escapes, public files, oversized values,
  malformed schemes, and foreign provider names fail closed.
- Production wiring remains responsible for selecting the projected external
  provider; this adapter is not a fallback for production configuration.
