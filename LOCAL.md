# Local Models

This guide is for running Prodex against a self-hosted local model instead of the OpenAI/Codex profile pool.

Use this path when you want the Prodex/Codex front end, Super mode, and local inference from a server you run yourself.

## What Prodex Expects

`prodex super --url` routes Codex through Prodex's local Smart Context rewrite proxy to a local OpenAI-compatible server. It does not use quota preflight or account auto-rotate.

For generic model aliases, the local server should provide:

- An OpenAI-compatible `/v1` base URL
- `POST /v1/responses` support
- Function-style tool definitions in the Responses API
- A model/template that can emit parseable tool calls

Codex CLI 0.124.0 and newer no longer supports the old `wire_api = "chat"` provider path. Generic model aliases still require Responses support. The qualified Offrail/Bonsai profile below uses a Prodex-owned Chat Completions adapter; Codex itself continues to use `wire_api = "responses"`.

## Quick Command

Start your local model server first, then run:

```bash
prodex super --url http://127.0.0.1:8131
```

Prodex appends `/v1` when the URL has no path, injects a temporary `prodex-local` Codex provider, disables non-function native tools that many local servers reject, and skips account quota/rotation routing.

The default local model id is:

```text
unsloth/qwen3.5-35b-a3b
```

Override it when your server exposes a different alias:

```bash
prodex super --url http://127.0.0.1:8131 --model local/my-model
```

## Offrail / Bonsai gaming-GPU launcher

For the `offrail-27b` and `bonsai-2-27b` aliases published by
[local-ai-on-your-gaming-gpu](https://github.com/sudoingX/local-ai-on-your-gaming-gpu),
Prodex selects the local Chat Completions compatibility profile automatically:

```bash
prodex s \
  --url http://127.0.0.1:8899 \
  --model offrail-27b \
  --context-window 169984 \
  --auto-compact-token-limit 150000
```

Use the context size actually configured on your server; the example is not a
hardware-independent context recommendation. An explicit model and numeric
context/compaction budgets outrank inherited configuration, including a trusted
project located at the user's home. You do not need to reselect the model in
`/model`. Global configuration is not rewritten.

This template supports `low`, `medium`, and `xhigh`. The default is `medium`;
unsupported inherited global effort is replaced with that default. An explicit
supported `-c model_reasoning_effort='"xhigh"'` is retained. Explicit `high` or
`max` fails with a supported-effort diagnostic before inference. Invalid effort
arriving through the local proxy returns HTTP 400, not a retryable overload.

The adapter calls `/v1/chat/completions` and converts function-call streams back
to Responses events. It translates freeform `apply_patch` and namespaced MCP
schemas/results rather than dropping them. Tools are advertised eagerly instead
of depending on unsupported Responses `tool_search`. Trusted system/developer
instruction fragments are combined into one initial system message for the
strict template; user, assistant, and tool-result ordering remains unchanged.
Local upstream requests do not inherit cloud account authorization headers.
Native `/compact` uses the same adapter, preserving history and the backend's
parallel-tool control before continuing the conversation.

Resume a saved **local-provider** conversation with the same explicit settings:

```bash
prodex s <SESSION_UUID> \
  --url http://127.0.0.1:8899 \
  --model offrail-27b \
  --context-window 169984 \
  --auto-compact-token-limit 150000
```

The in-app `/resume` picker uses the same local adapter. The explicit local URL
selects a stateless Chat Completions backend: Codex replays the saved transcript,
not a hosted-provider response cache. Historical session-only binding tombstones
must not block that replay. This exception does not apply to opaque response IDs,
turn-state continuations, hosted providers, or gateway tenant traffic. Such
continuations remain bound to their original provider and endpoint.

The adapter emits an actual Responses reasoning item before summary deltas and
closes it before answer/tool items. Plaintext reasoning survives tool-result and
saved-session replay; encrypted hosted-provider reasoning is not decrypted or
silently treated as plaintext. Codex can display the model-provided reasoning
while it streams, and `prodex log` reads the persisted reasoning. A log follower
started before a short new conversation picks up its bounded complete tail on
the next session-discovery tick; it does not replay an entire historical file.
No synthetic reasoning is added to responses that contain none.

RTK, Codebase Memory MCP, Playwright MCP, and Ponytail retain Super's automatic
activation. Presidio still asks for consent. Missing/incompatible external tools
retain their existing launch diagnostics. Other local model aliases and cloud
providers keep their existing protocol behavior.

## Authenticated endpoints and Cloudflare Tunnel

`--api-key` works with `--url` for fresh sessions and UUID/picker resumes.
The key is sent as `Authorization: Bearer ...` to the selected upstream for
inference, streaming, and compaction. Codex model discovery continues to use
Prodex's configured catalog locally. Explicit `--api-key`
uses the supplied value; otherwise local mode reads `PRODEX_LOCAL_API_KEY`.
It never falls back to `OPENAI_API_KEY` or a saved hosted-provider credential.

Prefer the dedicated environment variable so the key is not placed in the
process argument list or shell history:

```bash
read -rs -p "Local endpoint API key: " PRODEX_LOCAL_API_KEY
printf '\n'
export PRODEX_LOCAL_API_KEY

prodex s \
  --url https://llm.example.com \
  --model offrail-27b \
  --context-window 169984 \
  --auto-compact-token-limit 150000

unset PRODEX_LOCAL_API_KEY
```

The explicit flag is also supported, including after a session UUID:

```bash
prodex s <SESSION_UUID> \
  --url https://llm.example.com \
  --model offrail-27b \
  --context-window 169984 \
  --auto-compact-token-limit 150000 \
  --api-key "$OFFRAIL_KEY"
```

In the flag example, `OFFRAIL_KEY` must already hold the key; its expanded value
can be visible to process-list inspection. Prodex keeps local credentials in
the proxy process, not in Codex configuration, saved session metadata, or
runtime diagnostics, and removes the dedicated environment variable from
normal Codex/optional-tool child launches. Requests do not follow redirects;
configure the final HTTPS endpoint rather than a login/redirect URL. A `401`
from the server remains an authentication error, not a quota/overload signal.

Cloudflare Tunnel transports the API connection; it does not generate or
configure the model server's API key. Configure the server to require the same
key, use HTTPS for a remote hostname, and verify SSE streaming on that route.
Cloudflare Access login/service-token headers are separate from the model API
Bearer key and are not provided by this flag. Do not expose an unauthenticated
model server publicly.

## Example: llama.cpp Server

Install a recent `llama.cpp` build with a backend suitable for your GPU. For NVIDIA systems without a CUDA toolkit, the Vulkan build is often the simplest option.

Example launch:

```bash
llama-server \
  -m "$HOME/.local/share/prodex/models/Qwen3.5-35B-A3B-UD-Q3_K_XL.gguf" \
  --host 127.0.0.1 \
  --port 8131 \
  --alias "unsloth/qwen3.5-35b-a3b" \
  -ngl 20 \
  -t 8 \
  -c 16384 \
  -b 256 \
  -ub 256 \
  --parallel 1 \
  -fa on \
  --jinja \
  --chat-template chatml \
  --keep 1024 \
  --cache-type-k q8_0 \
  --cache-type-v q8_0 \
  --reasoning off
```

Adjust these values for your machine:

- `-m`: path to your GGUF model
- `--alias`: model id Prodex/Codex will request
- `-ngl`: number of layers offloaded to GPU
- `-c`: context size
- `-t`: CPU threads
- `--port`: local server port
- `--chat-template chatml`: avoids strict Qwen templates that reject Codex's message ordering

For an RTX 3060 12 GB with about 32 GB system RAM, `Qwen3.5-35B-A3B-UD-Q3_K_XL.gguf`, `-ngl 20`, `-c 16384`, `-b 256`, and `-ub 256` is a conservative mixed GPU/CPU starting point. The first Codex/Super prompt can be about 9,000 tokens before user context, so `-c 8192` is usually too small for this mode. If VRAM is tight, lower `-ngl` first. If compute buffers are still tight, lower `-b` and `-ub` before lowering context.

## Optional Wrapper Scripts

You can keep local model launch details out of shell history with wrapper scripts.

Server wrapper:

```bash
#!/usr/bin/env bash
set -euo pipefail

MODEL_PATH="${PRODEX_LOCAL_MODEL_PATH:-$HOME/.local/share/prodex/models/model.gguf}"
HOST="${PRODEX_LOCAL_HOST:-127.0.0.1}"
PORT="${PRODEX_LOCAL_PORT:-8131}"
MODEL_ALIAS="${PRODEX_LOCAL_MODEL_ALIAS:-local/model}"
GPU_LAYERS="${PRODEX_LOCAL_GPU_LAYERS:-20}"
THREADS="${PRODEX_LOCAL_THREADS:-8}"
CTX="${PRODEX_LOCAL_CONTEXT:-16384}"
BATCH="${PRODEX_LOCAL_BATCH:-256}"
UBATCH="${PRODEX_LOCAL_UBATCH:-256}"
CHAT_TEMPLATE="${PRODEX_LOCAL_CHAT_TEMPLATE:-chatml}"

exec llama-server \
  -m "$MODEL_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  --alias "$MODEL_ALIAS" \
  -ngl "$GPU_LAYERS" \
  -t "$THREADS" \
  -c "$CTX" \
  -b "$BATCH" \
  -ub "$UBATCH" \
  --parallel 1 \
  -fa on \
  --jinja \
  --chat-template "$CHAT_TEMPLATE" \
  --keep 1024 \
  --cache-type-k q8_0 \
  --cache-type-v q8_0 \
  --reasoning off
```

Prodex wrapper:

```bash
#!/usr/bin/env bash
set -euo pipefail

PORT="${PRODEX_LOCAL_PORT:-8131}"
MODEL_ALIAS="${PRODEX_LOCAL_MODEL_ALIAS:-local/model}"

exec prodex super \
  --url "http://127.0.0.1:${PORT}" \
  --model "$MODEL_ALIAS" \
  --context-window "${PRODEX_LOCAL_CONTEXT:-16384}" \
  --auto-compact-token-limit "${PRODEX_LOCAL_AUTO_COMPACT_LIMIT:-14000}" \
  "$@"
```

## Required Tests

Check the server sees your model:

```bash
curl -sS http://127.0.0.1:8131/v1/models
```

Check raw Responses API:

```bash
curl -sS http://127.0.0.1:8131/v1/responses \
  -H 'Content-Type: application/json' \
  -d '{"model":"local/model","input":"Reply with exactly: LOCAL-OK","max_output_tokens":32}'
```

Check Prodex text path:

```bash
prodex super --url http://127.0.0.1:8131 --model local/model \
  exec "Reply with exactly: PRODEX-LOCAL-OK"
```

Check tool use, not just text generation:

```bash
token="LOCAL-TOOL-$(date +%s)-$RANDOM"
printf '%s' "$token" > /tmp/prodex-local-tool-test.txt
prodex super --url http://127.0.0.1:8131 --model local/model \
  exec "Read /tmp/prodex-local-tool-test.txt using available tools. Reply with exactly the file content, no extra text."
```

The final output should match the token. If the model replies with a command such as `cat /tmp/...` instead of the file contents, text generation works but tool use is not working.

## Tool Templates

Local coding-agent use depends on tool calling. A model can answer simple prompts while still failing as an agent if its chat template does not tell it how to emit tool calls.

For `llama.cpp`, prefer a model/template combination that supports tool calls. If the model's bundled template is too strict for Codex message ordering, provide a custom template with `--chat-template-file`.

One common Qwen failure looks like this in the server log:

```text
Jinja Exception: System message must be at the beginning.
```

This means the model template rejected Codex's message order. Fix by starting `llama-server` with `--chat-template chatml`, a compatible custom Jinja template, or a newer model/server template that accepts later system/developer messages.

Another common failure:

```text
'type' of tool must be 'function'
```

This means the local server rejected a non-function native tool type. `prodex super --url` disables the common non-function tools for this path, but custom Codex configs or older Prodex builds may still hit it.

## Troubleshooting

If Codex prints a generic high-demand or reconnect message while using a local server, inspect the local server log. Codex may be translating a local `500` or parser error into a generic retry message.

If the server is slow on first request, that is normal. Large local models need to process the full Codex instruction and tool prompt, often several thousand tokens. Later requests may be faster if the server prompt cache is active.

If VRAM is exhausted:

```bash
PRODEX_LOCAL_GPU_LAYERS=20 ./your-local-server-wrapper
```

If Vulkan still fails while computing the first prompt:

```bash
PRODEX_LOCAL_BATCH=128 PRODEX_LOCAL_UBATCH=128 ./your-local-server-wrapper
```

If RAM or context memory is exhausted:

```bash
PRODEX_LOCAL_CONTEXT=16384 ./your-local-server-wrapper
```

If the model gives normal text but never uses tools:

- Verify `/v1/responses` receives a `tools` array
- Use a tool-capable chat template
- Try a smaller/faster model first to iterate on template behavior
- Keep `--reasoning off` if thought tags interfere with tool parsing

## What Local Mode Does Not Do

Local mode does not auto-rotate between Prodex profiles. There is no quota preflight, profile scoring, or OpenAI account fallback.

Local mode also does not make a weak local model behave like a hosted frontier coding model. The front end can expose tools, but the model still has to choose and format tool calls correctly.
