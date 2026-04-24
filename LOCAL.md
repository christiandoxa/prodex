# Local Models

This guide is for running Prodex against a self-hosted local model instead of the OpenAI/Codex profile pool.

Use this path when you want the Prodex/Codex front end, Caveman/Super mode, and local inference from a server you run yourself.

## What Prodex Expects

`prodex super --url` points Codex directly at a local OpenAI-compatible server. It does not use Prodex quota preflight, auto-rotate, or the runtime proxy.

The local server should provide:

- An OpenAI-compatible `/v1` base URL
- `POST /v1/responses` support
- Function-style tool definitions in the Responses API
- A model/template that can emit parseable tool calls

Codex CLI 0.124.0 and newer no longer supports the old `wire_api = "chat"` provider path. Servers that only implement `/v1/chat/completions` are not enough for `prodex super --url`.

## Quick Command

Start your local model server first, then run:

```bash
prodex super --url http://127.0.0.1:8131
```

Prodex appends `/v1` when the URL has no path, injects a temporary `prodex-local` Codex provider, disables non-function native tools that many local servers reject, and skips quota/proxy routing.

The default local model id is:

```text
unsloth/qwen3.5-35b-a3b
```

Override it when your server exposes a different alias:

```bash
prodex super --url http://127.0.0.1:8131 --model local/my-model
```

## Example: llama.cpp Server

Install a recent `llama.cpp` build with a backend suitable for your GPU. For NVIDIA systems without a CUDA toolkit, the Vulkan build is often the simplest option.

Example launch:

```bash
llama-server \
  -m "$HOME/.local/share/prodex/models/Qwen3.5-35B-A3B-UD-Q3_K_XL.gguf" \
  --host 127.0.0.1 \
  --port 8131 \
  --alias "unsloth/qwen3.5-35b-a3b" \
  -ngl 28 \
  -t 8 \
  -c 32768 \
  -b 512 \
  -ub 1024 \
  --parallel 1 \
  -fa on \
  --jinja \
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

For an RTX 3060 12 GB with about 32 GB system RAM, `Qwen3.5-35B-A3B-UD-Q3_K_XL.gguf`, `-ngl 28`, and `-c 32768` is a workable mixed GPU/CPU starting point. If VRAM is tight, lower `-ngl` first. If memory is still tight, lower `-c` to `16384` or `8192`.

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
GPU_LAYERS="${PRODEX_LOCAL_GPU_LAYERS:-28}"
THREADS="${PRODEX_LOCAL_THREADS:-8}"
CTX="${PRODEX_LOCAL_CONTEXT:-32768}"

exec llama-server \
  -m "$MODEL_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  --alias "$MODEL_ALIAS" \
  -ngl "$GPU_LAYERS" \
  -t "$THREADS" \
  -c "$CTX" \
  -b 512 \
  -ub 1024 \
  --parallel 1 \
  -fa on \
  --jinja \
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

exec prodex super --url "http://127.0.0.1:${PORT}" --model "$MODEL_ALIAS" "$@"
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

This means the model template rejected Codex's message order. Fix by using a compatible custom Jinja template or a newer model/server template that accepts later system/developer messages.

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
