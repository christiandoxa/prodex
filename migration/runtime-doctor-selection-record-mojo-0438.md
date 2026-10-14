# Runtime-doctor selection record ownership

This checkpoint moves the selection-event counter destination and rejection
reason precedence to compiled Mojo. A new versioned operation,
`prodex_mojo_runtime_doctor_selection_record_plan_v1`, reuses the existing
Mojo marker classifier and plans whether an event is selected, rejected,
or ignored. It also selects the first present field in the order
`reason`, `outcome`, `event`, `status`, or the marker as the final
fallback value. Empty but present fields keep their historical priority.

The production `parsing/selection.rs` caller no longer contains the
Rust semantic branches choosing these outcomes. Its remaining work is
collecting borrowed input presence, updating owned diagnostic counters,
rendering the existing timeline detail, and materializing strings. The
test-only marker-label mapper is not compiled into production, and there
is no Rust fallback implementation.

## Local validation

- Required-Mojo direct test: `selection_record_plan_checks_marker_and_reason_source_precedence`
  passed; verifies selected/rejected/ignored plans and invalid ABI, flags,
  and capacity.
- Required-Mojo caller regression:
  `selection_summary_counts_and_rejection_reason_priority_are_mojo_owned`
  passed, including blank reason, field precedence, unknown markers, and
  named profile and route aggregation.
- Full required-Mojo `prodex-runtime-doctor --features mojo --lib`:
  50 passed.
- All ownership, authority, no-fallback, and broad production-share
  guards passed.
- `cargo fmt --all` and `git diff --check` passed.

The 75% broad production-source project target is not yet met; this is
a behavior-preserving, production-reachable incremental checkpoint.
