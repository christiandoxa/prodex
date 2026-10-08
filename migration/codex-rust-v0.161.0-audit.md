# Codex rust-v0.161.0 compatibility audit

## Disposition and scope

Prodex 0.435.9 qualifies the OpenAI Responses HTTP/WebSocket, compaction metadata,
and app-server initialization boundaries against Codex `rust-v0.161.0`.
This updates a compatibility reference; it does not install Codex, raise Prodex's
capability-based minimum, or make Prodex the owner of Codex retry/compaction policy.

The exact tagged source contains a cross-OS remote-MCP limitation described below.
Qualification is not a claim that every upstream feature is regression-free.

## Provenance

- Repository: https://github.com/openai/codex
- Stable release: https://github.com/openai/codex/releases/tag/rust-v0.161.0
- Published: 2026-10-07T15:58:45Z; not a draft or prerelease.
- Annotated tag object: `7e21416b38834816c224ea0dfd135c3de94b2f15`.
- Peeled commit: `979011409de0a60b52f179721948e65531d26144`.
- Previous target: `rust-v0.160.1` at `d27764b82f7118f674371e6d6e76271d9d606edb`.
- Exact tagged-source archives differ in 1,195 files. The GitHub compare API returns
  only 300 files for this comparison; it was not used as a complete file inventory.
- Review scope: changed critical boundary owners, tagged-source marker replay,
  official executable digests, isolated initialization, and Prodex boundary tests.
  This is not an audit of every changed upstream implementation.

| Downloaded artifact | SHA-256 |
| --- | --- |
| codex-x86_64-unknown-linux-musl.tar.gz | `b1efb95097660d7f2e5a3887618a23f2ea1b0d548078bf92b0f7a5d229a0cef2` |
| codex-app-server-x86_64-unknown-linux-musl.tar.gz | `8e8a2e3b4f12536dd19db45ba61cbe5b8a5637ed86502463159d6de24e2b1035` |
| source-01601.tar.gz | `88baf0aba664474e58de98202d887ee803a8c954613ff58d1454a99e178488fc` |
| source-0161.tar.gz | `2dcc40af32e3529fd8e3184e3e5837873a92a5053ac3ef09e23e5ec6e792dba5` |

Binary archive hashes match the official GitHub release asset digests. Source
archive hashes record the exact fetched inputs rather than an upstream signature.

## Reviewed boundary changes

### Responses retry and transport fallback

`core/src/responses_retry.rs` keeps server retry guidance as a monotonic deadline.
Sampling and compaction share retry handling. A WebSocket-to-HTTP fallback waits
for server advice before changing transport; ordinary retry counts remain bounded.
Exhausted retry advice is attached to the originating turn rather than being reused
across unrelated turns. `codex-api/src/endpoint/responses_websocket.rs` now extracts
`RetryAfter` from HTTP upgrade failure headers instead of discarding that metadata.

Prodex preserves upstream error metadata. Its separate 0.435.9 local-capacity fix
pauses only the time spent waiting for local permits and does not erase upstream
attempt counts, failures, or exclusions. It does not implement Codex's fallback loop.
The baseline now requires retry/fallback deadline markers, and a negative validator
self-test rejects removal of the server-directed fallback deadline marker.

### Request order and continuation identity

`codex-api/src/common.rs` serializes `model`, `stream`, and `service_tier` before
potentially large input for both HTTP and WebSocket requests. WebSocket
`previous_response_id` remains optional and retains its value. This is an object
field-order change, not permission to drop or reconstruct continuation context.
New source checks retain both serializer layouts and the continuation field.

Compaction retains user-goal message identities in compacted-history metadata.
App-server decoding tolerates additive error strings/objects. These remain Codex
owned; Prodex does not interpret or rewrite goal state or add a competing error enum.

### Bedrock catalog normalization

The tagged catalog no longer removes Ultra reasoning or forces multi-agent V1.
It retains advertised model capabilities while continuing to normalize unsupported
service tiers, multimodal search, responses-lite, and tool-mode fields. Baseline
markers now guard the actual capability-preserving normalization, including V2,
rather than falsely requiring the removed suppression.

## Known upstream limitation: cross-OS remote stdio MCP

The exact 0.161.0 `rmcp-client/src/stdio_server_launcher.rs` lacks the 0.160.1
explicit `SYSTEMROOT`, `TEMP`, and `TMP` addition to the remote environment allowlist.
The default environment lists in `rmcp-client/src/utils.rs` are unchanged. Therefore
this audit does **not** claim preservation of that 0.160.1 fix for a Unix
orchestrator launching an MCP child on a Windows executor with explicit remote
environment variables.

The current baseline records the exact default-plus-explicit allowlist and retains
the regression assertion rejecting `UNREQUESTED_SECRET`. Historical 0.160.1 audit
and patch-delta evidence remain intact. Prodex does not replace installed Codex or
silently broaden a user's environment/secret allowlist. Users relying on that
cross-OS scenario should retain their known-working Codex version until the upstream
fix is present; it is separate from the reported local OpenAI fanout failure.

## Verification

- Exact tagged-source replay: 64 critical-file groups, 68 semantic groups,
  1,036 required marker occurrences, zero missing files or markers.
- Official Linux musl CLI and app-server archives match GitHub asset digests.
- The official CLI reports `codex-cli 0.161.0`.
- Isolated official app-server stdio `initialize` with `experimentalApi=true`
  succeeds and returns `codexHome`, `platformFamily`, `platformOs`, and `userAgent`.
  Synthetic HOME/CODEX_HOME and no provider credentials were used; no model turn
  or cost-bearing provider request was made.
- Prodex runtime verification for the hotfix: 406 internal runtime-proxy tests,
  nine focused retry/wakeup tests, and a 34-request WebSocket fanout regression.
- Actual Prodex binary with a loopback mock upstream: 128/128 requests at concurrency
  32 and 68/68 slow-upstream requests at concurrency 34, with zero request errors.
  These are local boundary/load results, not a live OpenAI capacity guarantee.

## Prodex 0.436.0 launch-boundary follow-up

The follow-up keeps the 0.435.9 transport qualification and fixes a concrete
Prodex launch bug exposed by the new global `codex exec --cyber-access-program`
option. `mojo/prodex_core/launch_args_common.mojo` is the canonical option-arity
owner. Without the new value-taking option, the shared planner interpreted its
value as a positional argument. This caused three production failures:

- `exec --cyber-access-program standard resume <id>` missed the resume command,
  and a program placed before the session could become the apparent session ID.
- Governed proxy configuration could be inserted between the option and its
  value, producing malformed Codex arguments before any provider request.
- Retry/goal recovery could discard the program value as if it were an old prompt,
  leaving a dangling option while retargeting the actual session.

The official binary accepts `standard`, `daybreak_blue`, and `daybreak_red`; the
CLI contract uses snake_case, not hyphens. Source guards retain that enum naming
attribute, and the valid-value regressions use the official spellings.

The fix adds the option to that existing Mojo owner. The Rust launch boundary
continues to borrow native arguments, validate the ABI result, and reconstruct
its output; no Rust parser, program enum, or fallback was added.

### Production callers and regression evidence

`prodex-runtime-launch/src/args.rs` uses the compiled Mojo planner for resume
inspection, governed configuration insertion, and `retarget_codex_exec_resume_args`.
The app's `app_commands/runtime_launch/resume_provider.rs` consumes the resulting
session identity to restore provider/model settings. `runtime_tools` recovery and
sub-agent launch paths use the same planner, so this is not a CLI-only helper fix.

The five new runtime-launch tests were first executed with the old Mojo source:
four failed and the inline-option control passed. After the one-line production
fix, all 69 runtime-launch tests passed. Cases cover every supported program value,
global positions around `exec resume`, fresh/resumed recovery, explicit Daybreak
settings, inline options, literal separator tails, missing values, and invalid
program text that resembles a session UUID.

CLI regressions also cover root/run/Super passthrough, exact explicit-setting
order, no generated Daybreak opt-in, and explicit GPT-6.1 Sol/Ultra forwarding.
Compatibility markers tie the disabled-by-default value to the complete
`Feature::CliDaybreak` block, not an unrelated false default elsewhere in the file.
A negative guard self-test rejects loss of explicit per-turn-program precedence.

### Feature eligibility is not transport passthrough

In the exact tag, `core/src/cyber_access_program.rs::for_provider` filters programs
to `OPENAI_PROVIDER_ID`. `exec/src/daybreak.rs` also requires that built-in provider
for automatic Daybreak routing. Prodex's existing governed transport identifies as
`prodex-openai-governed-http` to preserve Codex-owned HTTPS account bootstrap.
Therefore this release qualifies argument/configuration preservation, **not**
Cyber/Daybreak eligibility on that custom-provider path: Codex can omit an explicit
program there, and an enabled Daybreak request can be rejected by Codex.

Prodex does not bypass provider/account eligibility, inject entitlement metadata,
or duplicate upstream access-program selection to make the feature appear active.
`features.cli_daybreak` stays opt-in; `daybreak=true` alone does not enable it.
Existing saved preferences and explicitly chosen model/reasoning settings remain
untouched. The cross-OS remote-MCP limitation above also remains applicable.

### Isolated official-binary checks

The 0.436.0 follow-up independently compared both Linux archive hashes with the
current GitHub release asset digests and ran the official 0.161.0 app-server in a
fresh credential-free HOME/CODEX_HOME. `initialize`, `config/read`, and
`experimentalFeature/list` succeeded. The live feature list reports `cli_daybreak`
as under-development with both `enabled` and `defaultEnabled` false. No model turn
or cost-bearing provider request was made by these control-plane checks.

The final focused suite passes 69 runtime-launch tests, 147 CLI unit tests, and
two CLI integration tests. The exact-source follow-up baseline covers 65 critical
file groups and 72 semantic groups; all 1,078 marker occurrences match the tagged
source. The original 0.435.9 verification counts above remain historical evidence.

The compiled Prodex candidate also produced real `run --dry-run` and
`Super --dry-run` launch plans in isolated homes. Their rendered arguments were
decoded and passed to the official Codex 0.161.0 CLI. All eight cases passed:
three valid snake_case program values reached Codex's unsupported-review
validation on each path, and a hyphenated invalid value was rejected by Codex
rather than being rewritten by Prodex. The Super cases contain the actual
Mojo-planned governance overrides without splitting the option/value pair.
This is a launch-boundary smoke, not a successful Cyber model turn; the sanitized
results are in `codex-rust-v0.161.0-launch-boundary-smoke.json` alongside this audit.
