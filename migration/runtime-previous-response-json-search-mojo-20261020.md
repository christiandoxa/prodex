# Runtime Proxy Structured Error JSON Search Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled rich_fallback.mojo owner now decides how to search
structured previous-response errors, which of the message/detail/error
string aliases to select, how to classify those errors, and when to
stop at the historical 2048-visited-node boundary. The versioned
prodex_mojo_previous_response_json_search_v1 ABI combines this bounded
depth-first traversal and the preexisting compiled structured-error
classifier. It preserves first-match precedence and the default text
for a not-found classification with no string message.

Rust retains Serde parsing, generic iterative acquisition of at most
2048 tree nodes, and borrowing the six field values directly from
each original object. Providing direct field views preserves the
complete fields of the final visited object even when its children
are beyond the traversal cutoff. Mojo alone decides alias priority,
whether the error is meaningful, and which original object/field the
caller should materialize. No large JSON payload is cloned or
reserialized for selection.

The previous Rust runtime_json_find generic depth-first search,
its 2048-visit policy loop, candidate error-message classifier and
invalid-response-ID search closure were removed without a Rust
semantic fallback. All output and malformed ABI tags fail closed.

## Verification

- All 418 prodex-runtime-proxy tests pass, including four new
  production-caller regressions for nested errors, alias type barriers,
  Unicode values, exact 2048-visit boundaries, field access on the
  final candidate object and first pre-order matching.
- Three direct compiled-core tests verify alias and classification
  priority, exact structured response-ID detection, bounded inputs,
  invalid versions/operations/pointers and no writes on bad late
  field tags.
- Direct Mojo ABI test validates the same typed field and result
  contract, and the Mojo source compiles successfully.
- No-fallback guard and self-test, Cargo format, diff check and
  canonical production-share non-regression check pass.

The official share rises from 35.828561551188166% to
35.83402093790177% (130,720 Mojo / 234,073 Rust production LOC).
The separate 75% project target remains unmet. Metric definitions,
historical release floor and non-regression baseline were not changed.
Remote CI green was not required to publish this checkpoint.
