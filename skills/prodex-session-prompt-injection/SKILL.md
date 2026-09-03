---
name: prodex-session-prompt-injection
description: Safely control and observe an already-running plain prodex s session through its default expose MCP bridge.
---

# Existing Prodex session bridge

Use `prodex s expose` from the same canonical workspace as the one plain
`prodex s` session. The default MCP tools are:

- `prodex_session_prompt_inject({"message":"..."})` to queue a follow-up;
- `prodex_session_output_read({"cursor":"...","wait_ms":5000})` to read
  later sanitized visible output.

The two tools share one target identity. Require the same live OS user,
canonical cwd, plain-session command, Prodex process birth identity, actual
Codex writer ancestry/birth identity, Codex home authority, source identity,
and thread UUID. Ambiguity, restart, source rotation, or a changed writer is a
fail-closed error; never guess from the newest rollout.

Modern Codex thread authority is the writer's open
`thread-writer-locks/<UUID>.lock`. Legacy authority is exactly one open
`rollout-...-<UUID>.jsonl`. If both are present, their UUIDs must agree. A
normal session must be persisted and queue-addressable before using the
supported Codex queue/app-server transport.

Output reads are bounded and cursor-based. Return only existing sanitized
user-visible assistant/tool/status transcript events. Never expose hidden
reasoning, instructions, credentials, queue payloads, or raw rollout JSON.
Never read a target PTY or `/dev/pts`, synthesize keystrokes, write SQLite
queue rows, or start another solver/writer.
