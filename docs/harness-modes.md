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

Standalone gateways may set `harness = "auto" | "native" | "minimal" | "evaluated"` under
`[gateway]` in `policy.toml`; an explicit `prodex gateway --harness ...` value takes precedence.

## Modes

| Mode | Resolution | Request shaping | Response or stream shaping |
|---|---|---|---|
| `auto` | Conservatively resolves to `native`. | None after resolution. | None. |
| `native` | Native. | None; preserves existing request bytes and headers. | None. |
| `minimal` | Minimal. | Prepends the versioned Prodex instruction to ordinary `/v1/responses` inference requests. | None. |
| `evaluated` | Explicit opt-in only. | Applies only a versioned policy matched from the already-selected provider/model. | Applies only the matched policy's bounded response and stream transforms. |

An explicit CLI or configuration value wins. Unknown values are rejected. Omitting `--harness`
selects `auto`, so existing behavior remains unchanged. `auto` never resolves to `evaluated`, and
`evaluated` does not also apply the Minimal instruction block.

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

## Evaluated Policies

`evaluated` implements the Phase 2 design as a bounded, versioned policy catalog. Prodex matches
policy eligibility from the provider and model that the normal launch or gateway path already
selected. It never chooses, replaces, or reroutes a provider or model. The selected model must be
known to the typed provider catalog; an unknown provider/model pair, an unmatched policy, or a
non-Responses route is an exact-byte no-op at the harness layer.

The current evaluated policies are:

| Provider | Evaluation | Scope | Request behavior | Response behavior |
|---|---|---|---|---|
| Anthropic | `anthropic-native-messages-roundtrip-v1`, version 1 | Known catalog models on ordinary `/v1/responses` | Translates the supported canonical Responses subset to native Anthropic `/v1/messages`. | Translates buffered native Messages responses and native SSE back to canonical Responses shapes. |
| Gemini | `gemini-native-tool-alias-roundtrip-v1`, version 1 | Known catalog models on ordinary `/v1/responses` | Reversibly aliases the canonical `exec_command` tool name to Gemini's `run_shell_command`. | Restores the canonical name in typed buffered and SSE tool calls. |

### Native Anthropic Messages

The native translator supports text, client function-tool declarations and history, tool results,
basic sampling, stop sequences, and output-token limits. API-key requests use `x-api-key`; OAuth
Bearer authentication is preserved; native requests include `anthropic-version: 2023-06-01`.
Unsupported or lossy request and response shapes reject locally instead of silently dropping or
downgrading fields. The same selected account, model, affinity, bounded pre-commit retry, and
no-mid-stream-rotation rules still apply. SSE ping and unknown housekeeping events do not change
model-visible content.

### Reversible Gemini Tool Alias

On requests, the exact name `exec_command` is changed to `run_shell_command` in tool declarations,
named tool choice, and continuation function-call history. Declaring both names is ambiguous and
fails closed with `tool-alias-collision`. The transform is idempotent.

On buffered responses and SSE events, only typed `function_call` and `custom_tool_call` names are
restored to `exec_command`. Unrelated `name` fields and payload content are not rewritten. This
bounded reverse transform is the current response-postprocessing implementation; Evaluated mode
does not provide a general response rewriting layer.

## CLI

Use harness modes only on commands that launch a capable local bridge or gateway:

```bash
prodex s --provider anthropic --harness native
prodex s --provider anthropic --model claude-sonnet-4-6 --harness evaluated
prodex s deepseek --harness minimal
prodex s gemini --model gemini-3.1-pro-preview --harness evaluated
prodex super --url http://127.0.0.1:8131 --harness minimal
prodex gateway --provider gemini --harness native
```

The generated [provider capability matrix](./provider-capabilities.md) lists the selectable modes,
default resolution, supported canonical routes, and shaping phases from the typed provider-core
catalog. Gateway provider contracts expose the same non-secret metadata. Runtime diagnostics log
only stable mode, source, reason, route, provider, policy, and applied/not-applied fields; they never
log request bodies, prompts, instructions, tool arguments, credentials, or account identifiers.

## Request Processing Order

For ordinary provider-bridge inference, processing is:

1. existing Prodex-owned canonical request processing;
2. the resolved harness request shaper or matched Evaluated policy;
3. the selected provider wire-format translator;
4. the unchanged pre-commit selection and upstream transport path;
5. provider response translation, followed only by matched Evaluated postprocessing.

Native returns the original request without JSON parsing or reserialization. Minimal parses only an
eligible canonical Responses request, updates `instructions`, and then hands the shaped canonical
request to the same provider translator. Evaluated parses only a matched policy's eligible fields;
unmatched traffic stays borrowed and unchanged.

## Diagnostics and Tests

Inspect the provider contract with `prodex gateway providers --json` or
`GET /v1/prodex/gateway/providers`. Use `prodex doctor --runtime` and the resolved runtime log for
`harness_resolution`, `harness_request_shape`, `harness_provider_policy`, and
`local_rewrite_provider_conformance_*` markers. A matched policy marker includes only the provider,
route, model, phase, evaluation ID/version, and applied state. Prompt and request content is
intentionally absent from these surfaces.

Focused catalog, capability, conformance, request/response translation, buffered/SSE alias
round-trip, exact-byte no-op, collision, and runtime continuity tests protect the evaluated
additions. These tests do not replace the existing affinity, bounded pre-commit retry, transport,
and stream-commit suites.

## Non-Goals and Safety Boundaries

Harness modes do not read repository instructions, run tools, emulate another agent runtime, bypass
approvals, or change provider/account selection. Evaluated policies remain explicit, versioned,
provider/model scoped, and fail closed where a transform would be ambiguous or lossy. Hard affinity,
pre-commit-only rotation, transport transparency, redaction, bounded hot paths, and no mid-stream
rotation remain unchanged.
