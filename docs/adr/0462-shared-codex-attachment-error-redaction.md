# ADR 0462: Shared Codex attachment errors redact paths

## Status

Accepted.

## Context

Shared Codex attachment persistence rewrites session references for pasted text
and clipboard images. Error context for session traversal and rewrite failures
included absolute paths, which can expose profile names, overlay attachment
locations, or pasted-text identifiers.

## Decision

Use stable generic error context for Codex session traversal, session metadata,
and session read/write failures in the attachment persistence path.

## Consequences

Attachment persistence errors remain actionable without leaking local session or
attachment paths through accidental display formatting.
