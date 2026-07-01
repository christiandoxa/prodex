# ADR 0595: Gateway HTTP Pagination Query Name Decoding

## Status

Accepted.

## Context

Gateway HTTP pagination rejects duplicate `limit` and `cursor` parameters before
building a domain `PageRequest`. The parser compared raw query parameter names,
so percent-encoded names such as `%6cimit` could bypass duplicate detection.

## Decision

The gateway HTTP boundary now percent-decodes query parameter names before
matching the supported pagination parameters. Values remain owned by the domain
cursor and page-limit parsers.

## Consequences

Encoded duplicate pagination controls fail closed with the existing redacted
pagination error envelope. No HTTP framework or URL parsing dependency is added.
