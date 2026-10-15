# Smart Context rewrite quality risk policy moved to Mojo

The production rewrite telemetry planner now delegates its nine independent
quality-risk signals to compiled `prodex_smart_context_rewrite_quality_risk_v1`
in `mojo/prodex_core/smart_context.mojo`. Mojo alone decides whether a
sample is unsafe based on upstream context errors, previous-response loss,
invalid tool continuation, missing artifacts, repeated tool calls,
model reread, corrective messages, test/build failure, or task incomplete.
A successful or unknown task completion does not by itself indicate risk.

The former Rust OR-policy implementation and its private Rust source file
(`normalization/rewrite_policy.rs`) were deleted. Rust only marshals
fixed-width counters, flags, and completion-state tags into the typed
versioned output. The existing Mojo telemetry planner consumes that output;
no semantic Rust fallback or duplicate production risk classifier exists.

The no-fallback guard still watches the production telemetry consumer.
Its three stale references to the deleted Rust file were removed rather
than disabling the guard or checking a nonexistent file.

## Strict local validation

- Required-Mojo classifier ABI tests: 2 passed, covering every risk trigger,
  positive/unknown completion, invalid flags, invalid count and pointers.
- Required-Mojo runtime-proxy telemetry regressions: 5 passed, including
  an added end-to-end case for all nine signals.
- Full required-Mojo runtime-proxy library suite: 434 passed.
- Clippy with `--all-targets -- -D warnings`: passed.
- Ownership, authority, no-fallback, and broad production-share guards:
  all passed.
- Broad production share: 33.42% (111,958 Mojo LOC / 335,025 LOC).
  The 75% long-term project target remains outstanding.
