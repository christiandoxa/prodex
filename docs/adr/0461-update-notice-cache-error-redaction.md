# ADR 0461: Update notice cache errors redact paths

## Status

Accepted.

## Context

The update notice cache lives under the resolved Prodex state directory. Cache
read, parse, write, and replace failures previously formatted absolute paths in
their error context, which can expose local usernames, profile names, or managed
state layout.

## Decision

Use stable generic error context for update check cache operations. The concrete
path remains available to code that deliberately inspects `AppPaths`.

## Consequences

Accidental display of update-check failures no longer leaks local filesystem
paths.
