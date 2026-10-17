# Session recent-workspace selection in Mojo

Recent session workspace selection now calls
`prodex_session_recent_workspace_plan_v1` in the compiled session-selector Mojo
module. Mojo owns newest-first ordering, stable ties, absolute-path filtering,
exact workspace deduplication, and the result limit. Rust retains filesystem
traversal, JSONL reads, modification-time and platform path facts, and
`PathBuf` materialization.

The required-Mojo ABI and session-store caller tests cover duplicate and
relative workspaces, stable ordering, limits, and invalid output capacity.

The helper was introduced after the frozen ownership baseline, so this wave
claims no frozen semantic LOC credit; the broad production-source report tracks
the current Rust/Mojo balance.
