# ADR 0526: Characterize combo fallback delimiters

## Status

Accepted

## Context

Explicit `combo:` provider fallback chains support multiple separators and case-insensitive de-duplication. The focused provider-core test covered comma and semicolon separators, but not the pipe/arrow separators or mixed-case duplicate handling.

## Decision

Add a provider-core assertion that `combo:gpt-5|GPT-5>gpt-4o` returns `["gpt-5", "gpt-4o"]`.

## Consequences

Explicit fallback chain parsing remains stable across supported delimiters without changing runtime behavior.
