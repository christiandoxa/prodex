# Harness Modes

Harness modes are an opt-in model-facing request-policy layer for local provider bridges. They do
not create another agent runtime: Codex still owns the agent loop, tools, sandbox, approvals,
skills, hooks, reconnect behavior, and TUI.

Three separate concepts are involved:

- A **provider** defines upstream transport, authentication, and capabilities.
- An **account profile** supplies credentials and participates in Prodex selection, quota rotation,
  and continuation affinity.
- A **harness mode** defines model-facing canonical request policy for one bridge or gateway
  instance.

The harness is resolved once when a bridge starts. Account selection, retry, and rotation do not
change it, and a continuation cannot switch harnesses mid-stream.

Standalone gateways may set `harness = "auto" | "native" | "minimal"` under `[gateway]` in
`policy.toml`; an explicit `prodex gateway --harness ...` value takes precedence.

## Version 1 Modes

| Mode | Resolution | Request shaping | Response or stream shaping |
|---|---|---|---|
| `auto` | Conservatively resolves to `native` in v1. | None after resolution. | None. |
| `native` | Native. | None; preserves existing request bytes and headers. | None. |
| `minimal` | Minimal. | Prepends the versioned Prodex instruction to ordinary `/v1/responses` inference requests. | None. |

An explicit CLI or configuration value wins. Unknown values are rejected. Omitting `--harness`
selects `auto`, so existing behavior remains unchanged.

Minimal changes only the top-level `instructions` field. It preserves the model, input, tools, tool
schemas, tool choice, parallel-tool setting, reasoning, streaming flag, metadata, continuation IDs,
and unknown fields. The insertion is idempotent. Unsupported structured `instructions` values fail
closed instead of being removed or coerced.

The prepended block is:

```text
[Prodex harness: minimal/v1]
Act as a focused coding agent. Inspect the relevant code before editing, use
available tools to make concrete progress, keep changes minimal, and run the
smallest relevant verification before finishing.
```

Minimal does not apply to `/responses/compact`, model listing, embeddings, gateway admin routes,
websocket frames, response bodies, or stream events. It does not scan repositories, read
`AGENTS.md`, load skills, run hooks, rename tools, emulate vendor headers, bypass approvals, or alter
Prodex affinity, pre-commit rotation, retry, or streaming behavior.

## CLI

Use harness modes only on commands that launch a capable local bridge or gateway:

```bash
prodex s --provider anthropic --harness native
prodex s deepseek --harness minimal
prodex super --url http://127.0.0.1:8131 --harness minimal
prodex gateway --provider gemini --harness native
```

The generated [provider capability matrix](./provider-capabilities.md) lists the selectable modes,
default resolution, supported canonical routes, and shaping phases from the typed provider-core
catalog. Gateway provider contracts expose the same non-secret metadata. Runtime diagnostics log
only stable mode, source, reason, route, provider, and applied/not-applied fields; they never log
request bodies, prompts, instructions, tool arguments, credentials, or account identifiers.

## Request Processing Order

For ordinary provider-bridge inference, processing is:

1. existing Prodex-owned canonical request processing;
2. the resolved harness request shaper;
3. the existing provider wire-format translator;
4. the unchanged pre-commit selection and upstream transport path.

Native returns the original request without JSON parsing or reserialization. Minimal parses only an
eligible canonical Responses request, updates `instructions`, and then hands the shaped canonical
request to the same provider translator.

## Troubleshooting

Inspect the provider contract with `prodex gateway providers --json` or
`GET /v1/prodex/gateway/providers`. Use `prodex doctor --runtime` and the resolved runtime log for
resolution and shaping markers. Prompt and request content is intentionally absent from these
surfaces.

## Phase 2 Design Note (Not Implemented)

A later, separately evaluated phase may add:

1. native Anthropic Messages translation;
2. reversible provider-native tool aliases;
3. optional provider/model inference backed by evaluations;
4. optional response postprocessing;
5. explicit capability and continuity tests for every addition.

Phase 2 must remain opt-in and preserve the same fixed-per-runtime harness, hard affinity,
pre-commit-only rotation, transport transparency, redaction, and hot-path constraints. Version 1
does not implement any of these features.
