# Kiro ACP tool-activity ledger policy in compiled Mojo

The Kiro ACP live turn-state collector now delegates its tool-activity ledger
decisions to a compiled Mojo owner: whether to emit another item, emit the
single terminal truncation marker, or ignore subsequent items; whether an
activity ID is within the 256-byte caching bound; whether an existing label
can be refreshed under the 128-event cap; and when missing title/kind fields
inherit values from the previous activity with the same bounded ID.

The previous Rust truncation threshold, ID-length check, previous-label
selection and record-label gating were deleted. Rust still owns the session
notification stream, BTreeMap/Vec data structures, and application of the
typed Mojo plan. It invokes the existing Mojo tool-activity renderer to
materialize each selected event. A missing rendered activity name now fails
closed instead of silently falling back to a Rust name.

Kiro's main Mojo module is imported by the rich kernel rather than linked as
a standalone object. Therefore the small versioned ledger owner is built as
`kiro_acp_tool_activity_policy.mojo`, explicitly included by `build.rs` in
the production static library; a strict guard requires this link entry.

Direct ABI tests cover all three actions, one truncation marker, event limits,
previous-label inheritance, 256-byte UTF-8 boundaries, corrupt flags and
invalid pointers/capacity. Real ACP caller tests verify truncated progress,
inherited title/kind, Unicode oversized IDs and absence of duplicate label
records. All 30 Kiro ACP app tests pass. The no-fallback guard prevents
restoration of the Rust ledger algorithm.

The independent long-term 75% Mojo production-share goal is still unmet.
