# 0426: Domain audit metadata display redaction

## Status

Accepted

## Context

Audit action, resource kind, resource ID, and reason code values are security
log metadata. API response planners already redact invalid values, but generic
error display text can be logged before response planning.

## Decision

Audit metadata validation errors no longer include rejected input lengths in
their `Display` output.

## Consequences

- Invalid audit metadata details stay out of generic error plumbing.
- Error response codes and messages remain unchanged.
