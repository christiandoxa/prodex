# Smart Context rewrite telemetry sample policy in compiled Mojo

The live runtime Smart Context telemetry caller now delegates sample inclusion,
rewrite success-state recognition, quality-risk classification, and safe token
savings classification to one versioned compiled Mojo operation. The existing
nine-signal quality-risk kernel is reused as the authoritative classifier,
including explicit failed task completion, missing artifacts, invalid tool
continuations, model rereads, and other post-rewrite evidence.

The previous Rust predicates for `potentially_rewritten`, `safe_saved`, and
`quality_risk` were deleted. Rust retains only acquisition of record fields,
typed ABI marshaling, and the construction and persistence of the resulting
telemetry sample. There is no Rust fallback for the migrated decisions.

Mojo preserves the exact `rewritten` / `minified` and `ok_saved` /
`ok_surgical_rehydrate` / `ok_minified` statuses, the existing behavior for
fallback-only samples, and strict token-saving semantics. The ABI validates
UTF-8 spans, nine quality metrics, flag tags, capacity, and version before
writing output. Direct tests cover each selection/safety case and invalid ABI
shapes. The real app caller tests and broader Smart Context suite cover
safe, skipped, failed, and quality-risk scenarios. The no-fallback guard
requires the ABI, caller coverage, and absence of Rust equivalents.

The independent 75% project production-share target remains incomplete.
