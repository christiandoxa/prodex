# 0621. Domain EntityTag Character Guard

## Status

Accepted

## Context

Entity tags are request-controlled precondition metadata used by mutating
control-plane operations. The domain boundary already rejected empty and
overlong values, but accepted whitespace, control characters, and non-ASCII
text.

Those values are risky in HTTP headers, logs, audit trails, and future OpenAPI
examples.

## Decision

`EntityTag::new` now accepts only ASCII graphic characters after trimming.
Whitespace, control characters, and non-ASCII values fail with
`EntityTagError::InvalidCharacter`.

The existing `plan_entity_tag_error_response` maps this to the same redacted
`entity_tag_invalid` response.

## Consequences

- Mutation precondition metadata is normalized at the domain boundary.
- Raw invalid characters and offsets stay out of public API errors.
- Gateway adapters still own HTTP header parsing before passing entity-tag text
  to the domain boundary.
