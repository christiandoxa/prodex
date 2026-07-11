# ADR 0851: Redact gateway HTTP metadata display errors

Status: Accepted

## Context

Gateway HTTP idempotency-key and entity-tag helpers distinguish duplicate
request metadata. Their `Display` output named specific headers, so generic
error formatting could expose header topology outside the stable response
planner.

## Decision

Keep typed duplicate errors for response planning, but render duplicate
idempotency-key and entity-tag metadata as a generic duplicate request metadata
message.

## Consequences

HTTP metadata parsing remains unchanged, while stringified local errors no
longer expose idempotency or entity-tag header names.
