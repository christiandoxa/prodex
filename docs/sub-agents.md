# Super sub-agents

`prodex super` (also `prodex s`) can configure bounded child Prodex processes for a Codex main agent. The model decides which work to delegate; Prodex supplies effective instructions, a shell-free child launcher, recursion prevention, Presidio and strict optional-tool inheritance, and a cross-process concurrency limit.

## Interactive flow

A fresh interactive launch resolves screens in this order:

1. Presidio decision.
2. Main-agent provider.
3. Required main-provider configuration, such as a local URL.
4. Use sub-agents?
5. Sub-agent provider.
6. Sub-agent local URL when the child provider is `local`.
7. Sub-agent model.
8. Sub-agent reasoning effort.
9. Maximum active sub-agents.
10. Launch.

Answering no at step 4 skips every child configuration screen. Explicit main-provider arguments skip step 2. Explicit `--sub-agent` skips the child wizard and uses defaults for omitted details.

Non-TTY execution never opens these screens. It uses explicit values and documented defaults, and fails before launch when a required provider value is missing.

## Main and child providers

Main and child provider choices use separate typed configuration. `--provider` or `--url` configures the parent. `--sub-agent-provider` and `--sub-agent-url` configure children and never change the parent.

The canonical provider registry currently exposes:

- OpenAI
- Anthropic Claude
- GitHub Copilot
- DeepSeek
- Google Gemini
- Kiro
- Prodex Local

Provider model pickers use the checked-in canonical catalog without requiring a network request. They include provider default, built-in models in recommendation order, configured/current models when present, and a custom-model entry. Lists scroll on short terminals. A nonempty custom model ID is preserved exactly even when it is absent from the catalog.

The main-agent OpenAI picker consumes the current top-level Codex catalog, including
catalog-visible `gpt-5.6-sol`, `gpt-5.6-terra`, and `gpt-5.6-luna` when the active
catalog exposes them. Prodex child selection remains a separate child-provider catalog;
it is not inferred from a truncated native task-tool description. Effort options are
resolved from the selected model's catalog metadata.

## Resume and affinity

These forms resolve the same parent resume target:

```bash
prodex s 00000000-0000-7000-8000-000000000042
prodex s resume 00000000-0000-7000-8000-000000000042
```

A session with recorded provider affinity stays bound to that parent provider. A conflicting explicit provider fails before launch. Child provider selection remains independent. The parent UUID is never written into child configuration, instructions, task files, or child arguments.

## CLI configuration

`--sub-agent` explicitly enables delegation. `--no-sub-agent` disables it. Child detail flags require `--sub-agent`, and the two decision flags conflict.

| Flag | Behavior |
| --- | --- |
| `--sub-agent-provider PROVIDER` | Defaults to OpenAI. Aliases resolve through the canonical provider registry. |
| `--sub-agent-model MODEL` | Optional nonempty model ID. Catalog entries are suggestions, not an allowlist. |
| `--sub-agent-model-reasoning-effort EFFORT` | Optional effort. Known models use declared efforts; dynamic or custom models use provider or standard choices. |
| `--sub-agent-url URL` | Credential-free absolute HTTP(S) URL, valid only for a local child provider. |
| `--sub-agent-max-concurrency VALUE` | Maximum simultaneously active official child processes. Accepts `default` or an integer from 1 through 64. |

The built-in concurrency default is 4. Presets are 4, 8, 16, and 32. A custom value may be any integer from 1 through 64. This is a simultaneous-process limit, not a lifetime limit on delegated tasks.

```bash
prodex s \
  --sub-agent \
  --sub-agent-max-concurrency default

prodex s \
  --sub-agent \
  --sub-agent-max-concurrency 8

prodex s \
  --sub-agent \
  --sub-agent-provider kiro \
  --sub-agent-model gpt-5.6-luna \
  --sub-agent-model-reasoning-effort max \
  --sub-agent-max-concurrency 16

prodex s \
  --sub-agent \
  --sub-agent-max-concurrency 23
```

Explicit 4 is pinned to four; `default` follows a future default change. Higher limits increase CPU, memory, I/O, provider quota, and rate-limit pressure.

The concurrency flag works before or after a resume UUID:

```bash
prodex s --sub-agent --sub-agent-max-concurrency 16 \
  00000000-0000-7000-8000-000000000042

prodex s 00000000-0000-7000-8000-000000000042 \
  --sub-agent --sub-agent-max-concurrency=16
```

## Runtime enforcement

Each temporary overlay contains exactly the configured set of exclusive lock slots. Every official launcher process acquires one OS-backed file lock before spawning a child and holds it for the complete child lifetime. Separate launcher processes share these slots. Locks release on normal exit, child failure, failed spawn, cancellation, signal handling, output failure, or abnormal launcher termination where the operating system releases process locks.

When every slot is active, the launcher immediately exits nonzero with:

```text
sub-agent concurrency limit reached; wait for an active child to finish before retrying
```

It does not spawn another child, wait forever, or busy-spin. Existing children remain untouched.

The supervisor separates process liveness from output activity. A child that is
still running but quiet— including a long reasoning interval, partial line, or
stderr-only progress— remains `Running`/`Idle but alive`; silence alone does not
trigger reconnect or restart. Stdout and stderr are drained concurrently with
bounded buffering, and pipe EOF is treated as terminal only with the child exit
or a provider contract that says the channel is closed. Provider retry, rate
limit waiting, authentication failure, cancellation, and actual child exit are
reported as distinct outcomes rather than one generic reconnect state.

## Effective instruction delivery

Prodex writes a diagnostic `SUB_AGENTS.md` and injects its complete content between these markers in the temporary overlay's effective Codex instruction file:

```html
<!-- PRODEX SUB-AGENT BEGIN -->
<!-- PRODEX SUB-AGENT END -->
```

A nonempty `AGENTS.override.md` is effective and receives the block. Otherwise `AGENTS.md` receives it. An empty override is skipped. Repeated activation replaces the marked block rather than duplicating it. Base profile files and repository instruction files are not modified.

## Shell-free child invocation

Prodex resolves its current executable with `std::env::current_exe()`. The overlay stores a bounded, private, credential-free child launch specification and a private task directory. The main agent writes one narrow task file, then invokes the hidden launcher with fixed paths, for example:

```text
'/opt/prodex/bin/prodex' '__sub-agent-exec' '--config' '/tmp/prodex-overlay/sub-agent-launch.json' '--task-file' '/tmp/prodex-overlay/sub-agent-tasks/task-001.txt'
```

That small display command is rendered for the host shell. Actual child construction uses an argument vector, never `/bin/sh -c`, PowerShell evaluation, or `cmd.exe /c`. Task text may contain spaces, apostrophes, quotes, newlines, Unicode, and shell metacharacters; it reaches `exec` as one exact argument.

The hidden `__sub-agent-exec` command accepts only `--config` and `--task-file`. Do not append public child flags such as `--no-sub-agent`, `--provider`, `--model`, or `exec` to that launcher command; Prodex constructs those arguments after reading the private config.

A safe representative child argument vector is:

```text
/opt/prodex/bin/prodex
s
--no-sub-agent
--presidio
--provider
copilot
--model
auto
-c
model_reasoning_effort=xhigh
exec
<exact task as one argument>
```

Exactly one of `--presidio` or `--no-presidio` is inherited, along with every parent `--require-tool` selection. `--no-sub-agent` plus `PRODEX_SUB_AGENT=1` prevents grandchildren. The launcher keeps stdout and stderr separate, preserves the real child status, consumes bounded task input, and never depends on `prodex` being in `PATH`.

## Dry run

`--dry-run` reports whether sub-agents are enabled, provider, model/default, effort/default, maximum active children and source, hard maximum, exclusive slot enforcement, inherited Presidio state, recursion prevention, and absence of parent UUID inheritance. URLs, credentials, private task contents, and parent UUIDs are not printed.

Examples include:

```text
Maximum active sub-agents: 4 (Prodex default)
Maximum active sub-agents: 16 (explicit preset)
Maximum active sub-agents: 23 (custom)
```

## MVP boundary

Implemented behavior covers configuration, provider/model/effort selection, effective instruction injection, deterministic child launch, cross-process active-child limits, Presidio and strict optional-tool inheritance, recursion prevention, and stdout/stderr/status propagation.

Prodex does not provide automatic work decomposition, runtime-enforced file ownership, distributed
supervision, remote model discovery, A2A child transport, or automatic worktree allocation. The
main model remains responsible for choosing narrow tasks, avoiding overlapping edits, verifying
child output, integrating results, and running final validation.
