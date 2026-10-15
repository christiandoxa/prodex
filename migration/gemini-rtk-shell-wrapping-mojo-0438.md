# Gemini RTK shell-command wrapping in Mojo

`prodex_gemini_rtk_shell_command_v1` now owns the Gemini raw-shell segment
scan, environment assignment handling, Unicode whitespace, noisy-command
and subcommand eligibility, and first-match RTK insertion. The previous
Rust scanner and duplicate command catalogue have been deleted.

Common command and subcommand policy is reused from `rtk_noisy.mojo`.
Gemini's existing differences are explicit: no executable unquoting,
no echo wrapping, and the existing compactor commands. This is a command
text transformation only; no command is executed and no tools installed.
Rust retains the existing Serde argument codec and complete String projection.

Required-Mojo evidence:
- 30,492 generated command combinations match the former Rust implementation.
  The temporary test reference is removed after execution.
- Direct Mojo ABI, short output, long path and fixed policy cases: 2 passed.
- New JSON tool-call caller regression: passed.
- Full provider-core library: 449 passed.
- Provider-core all-target Clippy with denied warnings: passed.
- Ownership, authority, no-fallback, size and production-share guards: passed.

The isolated checkpoint is 33.31% Mojo (111,276 / 334,083 production LOC).
The project target is still 75%; release metadata and metric rules are unchanged.
