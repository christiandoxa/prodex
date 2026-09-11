# ChatGPT MCP expose

The canonical detailed Expose guide is [EXPOSE.md](../EXPOSE.md). This page is
kept as a short documentation-index reference so links to `docs/expose.md`
remain useful without maintaining a second deep-dive.

## Modes

| Command | Browser | MCP |
| --- | --- | --- |
| `prodex s expose` | Local loopback | Local loopback |
| `prodex s expose --tunnel` | Public Cloudflare Quick Tunnel path | Public Cloudflare MCP path |
| `prodex s expose --tunnel-provider cloudflare` | Public Cloudflare path | Public Cloudflare MCP path |
| `prodex s expose --tunnel-provider openai` | Local loopback only | Remote through OpenAI Secure MCP Tunnel |

OpenAI Secure MCP Tunnel is MCP-only. It is not a public browser reverse proxy,
and Prodex does not emit a public browser URL for it.

## Minimal commands

```bash
prodex s expose
prodex s expose --tunnel
CONTROL_PLANE_TUNNEL_ID=tunnel_0123456789abcdefghijklmnopqrstuv \
CONTROL_PLANE_API_KEY='replace-with-your-runtime-key' \
prodex s expose --tunnel-provider openai
```

The OpenAI identifier is non-secret and must match the validated `tunnel_` plus
32 lowercase-letter/digit form. Keep `CONTROL_PLANE_API_KEY` outside argv and
shell history. `--openai-tunnel-id` is the non-secret CLI alternative to the
identifier environment variable; there is no `--openai-api-key` option.
TTY OpenAI setup prompts only for missing values; configured values skip the prompt.

## Standalone MCP execution

`prodex_super_exec` runs one direct command under the same local OS-user
authority as Expose. It does not require, create, attach to, or depend on a
Prodex Super run or a separate `prodex s` session. `program` and `args` are
passed without shell parsing; use `sh -c` or `cmd.exe /C` explicitly for shell
syntax.

```json
prodex_super_exec({
  "program": "python3",
  "args": ["-c", "print('hello from Python')"],
  "cwd": "/home/test-user/project",
  "env": {"MODE": "check"},
  "stdin": null,
  "timeout_ms": 30000
})
```

`cwd` defaults to the captured expose workspace. Timeout is bounded to
120 seconds, output is separately capped and secret-redacted, and results
include status, PID, exit code/status, signal, duration, stdout/stderr, and
truncation flags. Timeout or expose shutdown terminates and reaps the child
process tree.

## Existing-session MCP bridge

Run one plain `prodex s` and one matching `prodex s expose` from the same
canonical workspace. MCP tools are enabled by default:

```json
prodex_session_output_read({})
prodex_session_prompt_write({"message":"inspect the failing test"})
prodex_session_preempt({"thread_id":"<thread_id>"})
prodex_session_output_read({"cursor":"<next_cursor>","wait_ms":5000})
```

The tools share the same fail-closed resolver and bind input/output to the same
Prodex PID, Codex writer PID, canonical cwd, and thread UUID. They observe only
sanitized visible user, assistant, tool, MCP/agent, and session/turn-status
events, use bounded cursor reads, and use the
supported Codex app-server control plane for input. They never scrape or
write a PTY, insert SQLite queue payloads, or create a second solver. The
unmodified Codex TUI is not required to render externally queued follow-ups;
Output Read is the authoritative machine-readable mirror. Prompt Write may
return an optional `output_cursor` only when the exact pre-write rollout
source and checkpoint are known.
When the app-server control plane proves that the first attempt was rejected
before acceptance or failed during preflight, Prompt Write revalidates the same
process/writer/thread and retries once. Its result reports
`recovery_generation`, `last_prompt_requeued`, and `requeue_reason`. Accepted
and ambiguous outcomes are never retried.

Prompt Write returns machine statuses such as `written`, `no_session`,
`ambiguous_session`, `stale_target`, `queue_failed`, and `write_ambiguous`.
Only authoritative `no_session` permits a `prodex_super_start` fallback.
`write_ambiguous` means the request may have been accepted after a transport
close, timeout, or malformed response; never replay it automatically and make
no exactly-once claim. Output pages may contain bounded generic `gap` markers
for malformed, invalid-UTF-8, or oversized records. Bounded text carries a
`[text_truncated]` marker; impossible bounded recovery returns
`recovery_failed`.

Modern Codex authority is an open
`thread-writer-locks/<UUID>.lock`; legacy authority is one open
`rollout-...-<UUID>.jsonl`. If both exist, UUIDs must agree. A fresh Codex
0.153.2 session can have its thread lock before its first rollout row exists.
The bridge verifies that exact loaded thread through the writer's app-server
socket, writes through Codex, and requires persistence after writing; a queue
database and UUID alone are never sufficient.

`prodex_session_preempt` uses the same fail-closed target checks. It verifies
exact thread addressability and status with `thread/read`, obtains the exact
in-progress turn ID with `thread/turns/list`, and sends `turn/interrupt` only
for that turn. It then drains pending `thread/queue/*` submissions through
`thread/queue/list` and `thread/queue/delete`. Codex pauses queued submissions
when a turn is interrupted, so no pending submission starts between those two
operations. The queue-empty observation is the linearization boundary: bridge
Prompt Write calls before it are cancelled; calls accepted after it remain
valid. It returns cancelled and remaining submission IDs, interruption status,
`session_ready`, and a monotonic in-process generation boundary. Codex owns
persisted queue state, so reconnect must observe the queue through the app-server
rather than replaying a request. Already-started submissions are never counted
as cancelled. Ambiguous control responses fail closed.

## Security and readiness

The local origin binds to loopback. Expose validates local MCP `initialize` and
`tools/list` before reporting ready. Cloudflare additionally validates the
public MCP endpoint. Cloudflare Quick Tunnel uses `cloudflared --protocol auto`
with QUIC/UDP 7844 preferred and HTTP/2/TCP 7844 as the bounded fallback. A
registered transport is not the same as public DNS, TLS, or MCP application
readiness; see [EXPOSE.md](../EXPOSE.md#dns-doh-tls-and-mcp-are-separate-layers).

The printed MCP URL contains an ephemeral full-access bearer capability. Treat
it as a credential and stop the process to revoke it. This is not OAuth or a
multi-user authorization boundary.

## Focused validation

```bash
cargo test --locked -q -p prodex-app --lib expose:: -- --test-threads=1
cargo test --locked -q -p prodex-cli --tests expose
```

For CLI options, lifecycle, route isolation, troubleshooting, OpenAI
`tunnel-client` supervision, and the complete security model, use the root
[EXPOSE.md](../EXPOSE.md).
