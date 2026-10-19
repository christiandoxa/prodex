# DeepSeek SSE tool-call event and completion planning migrated to Mojo

The production DeepSeek-style SSE stream now delegates two related
decisions to the existing compiled DeepSeek stream-policy owner:
whether a newly observed tool-call index needs its output_item.added
event and whether a fragment should emit function_call_arguments.delta;
and which ordered tool-call indexes remain unfinished at finish_reason
or final stream completion.

The previous Rust event-added flag predicate, hard-coded special-tool
names tool_search/apply_patch, and BTreeMap done-state filtering
decision tree were removed. The compiled event planner reuses the
existing Mojo tool-name-kind classifier, and the completion planner
returns ordered pending positions after validating completion-state
flags. Rust remains responsible for Serde acquisition, tool-call
metadata and argument storage, RTK argument rewriting, output JSON
materialization, and applying the Mojo-selected state transitions.

The typed ABI validates UTF-8, presence flags, positions, buffer
capacities, versions, and malformed names before accepted results can
affect emitted events. Invalid Mojo output now causes an explicit
invalid_tool_call_arguments stream failure, not a Rust fallback
classifier. The host verifies that selected completion indices were
unfinished, unique and in stable original order before applying them.
No committed stream is replayed.

Three new direct tests cover the special-tool item/delta precedence,
normal and Unicode function names, repeat/add flags, mixed pending
tool positions, bad buffers and invalid ABI shapes without writes.
Production SSE tests cover interleaved fragments, invalid tool input,
tool_search, apply_patch, RTK wrapping and exactly-once completion
across repeated finish_reason chunks. All 24 targeted DeepSeek SSE
caller tests and 689 Mojo-core tests pass. Source-level no-fallback
guards prevent restoring the old Rust predicates.

This is production-reachable semantic ownership migration. The
independent 75% Mojo production source-share target remains unmet.
