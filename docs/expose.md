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

## Existing-session MCP bridge

Run one plain `prodex s` and one matching `prodex s expose` from the same
canonical workspace. MCP tools are enabled by default:

```json
prodex_session_output_read({})
prodex_session_prompt_write({"message":"inspect the failing test"})
prodex_session_output_read({"cursor":"<next_cursor>","wait_ms":5000})
```

The tools share the same fail-closed resolver and bind input/output to the same
Prodex PID, Codex writer PID, canonical cwd, and thread UUID. They observe only
sanitized visible user, assistant, tool, MCP/agent, and session/turn-status
events, use bounded cursor reads, and use the
supported Codex app-server control plane for input. They never scrape or
write a PTY, insert SQLite queue payloads, or create a second solver. Prompt
Write is observable in the already-open parent TUI; output read is a
complementary machine-readable mirror.

Modern Codex authority is an open
`thread-writer-locks/<UUID>.lock`; legacy authority is one open
`rollout-...-<UUID>.jsonl`. If both exist, UUIDs must agree. A fresh Codex
0.153.2 session can have its thread lock before its first rollout row exists.
The bridge verifies that exact loaded thread through the writer's app-server
socket, writes through Codex, and requires persistence after writing; a queue
database and UUID alone are never sufficient.

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
