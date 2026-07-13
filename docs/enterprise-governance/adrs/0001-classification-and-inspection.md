# ADR 0001: Classification and Inspection

- Status: Accepted
- Scope: target enterprise governance architecture

## Context

Governed routing and policy need a conservative data classification without
copying request content into decision, routing or audit records. Existing
Presidio and local guardrail paths overlap and coverage varies by schema and
modality.

## Decision

Use monotonic `public < internal < confidential < restricted` classification.
Only trusted rules/findings may raise classification; no untrusted input lowers
it. Inspection returns typed, bounded, content-free findings plus explicit
`full`, `partial` or `unsupported` coverage and detector/rule revisions.
Schema-aware walkers cover prompt fields and nested tool arguments, use correct
UTF-8 byte locations, enforce depth/value/size/finding limits, and mark opaque
or unsupported modalities honestly. A single inspection result feeds the
application boundary before policy, reservation and provider dispatch. Mode and
active policy determine fail-open, constrain or fail-closed behavior.

## Consequences

Downstream stages use stable metadata rather than raw content. Duplicate PII
authorities must be consolidated behind the typed result. Streaming responses
need an incremental bounded PEP and cannot claim full coverage before final
validation.

## Implementation status

The candidate implements the typed bounded inspection model, monotonic compiled
classification rules, schema-aware JSON walking, local detectors, bounded
Presidio mapping, explicit coverage and response-stream overlap inspection.
Evidence includes `inspection_result_is_bounded_deterministic_and_content_free`,
`application_inspection_combines_sources_monotonically`,
`local_inspection_rejects_deep_and_match_flood_inputs`, and
`incremental_inspector_finds_every_chunk_boundary`. Supported unary, SSE,
WebSocket and Gemini paths pass the common boundary. Presidio remains a bounded
request-path network dependency when policy selects it.
