# Smart Context

Smart Context is a conservative request rewrite inside the runtime proxy. It
preserves protocol and continuation fields and returns the original bytes when
a rewrite cannot be proved safe and token-positive.

## Current Rewrite Contract

The active engine currently performs one production rewrite: exact duplicate
text inside the same supported Responses request. The first occurrence stays
inline. Later occurrences become versioned inline references, and a
`developer` message explains deterministic local expansion. Validation expands
those references and compares the result with the original request before
commit.

Prodex does not emit new persisted-artifact references. Existing legacy
artifact references are accepted only when their content is durable, digest
verified, and available in the active scope. A missing mandatory reference
blocks the request before upstream; unsupported routes stay byte-identical.

## Fast Pass-Through

These paths return the borrowed/original body without serialization:

- Smart Context disabled;
- explicit `x-prodex-smart-context: exact`;
- canary-out;
- unsupported route or content type;
- body below the admission floor with no artifact reference;
- unsupported tokenizer;
- no-op, validation failure, or sub-threshold savings.

Shadow is sampled at 1% of eligible traffic. A sampled shadow request analyzes a
disposable state snapshot, returns the original bytes, and commits nothing.
Unsampled shadow requests decline before JSON parsing.

Active work is capped at 256 KiB for HTTP and 96 KiB for WebSocket. A release
rewrite that reaches 100 ms returns the original bytes instead of committing;
debug builds use 5 seconds because tokenizer execution is unoptimized. An
oversized request containing a mandatory artifact reference still enters the
fail-closed reference check rather than sending an unresolved reference
upstream.

Set `PRODEX_SMART_CONTEXT_CANARY_PERCENT=0` for an immediate pass-through kill
switch. This setting changes only new pre-commit decisions.

## Plan, Validate, Commit

For an eligible request Prodex:

1. parses the request once and reads the top-level model;
2. snapshots the scoped engine state;
3. builds a rewrite against the snapshot;
4. serializes and tokenizer-counts the candidate;
5. expands inline references and validates protocol fields, critical signals,
   JSON shape, and the safety margin;
6. returns the original bytes on any failure;
7. commits the pending state only after all checks pass.

Exact, canary-out, shadow, rejected, and fallback requests have zero Smart
Context state mutations. No lock is held while parsing, scanning, hashing,
serializing, tokenizing, or doing file I/O.

## Scope and Persistence

`ContextScopeId` binds tenant/root, profile, provider endpoint, canonical
workspace, and optional session. Persisted stores are scope-separated,
AES-256-GCM-SIV encrypted, private-file protected, size bounded, retained for
30 days, and subject to a 64 MiB global cap. Artifact identity is
`sc2:<sha256>`; reads verify schema, scope, digest, byte length, and exact
content.

Corrupt or wrong-scope stores are quarantined and reported as degraded. The
process lock registry keeps weak entries and removes unused paths. Artifact
recency uses a persisted order counter with digest tie-breaking, never a
request ID.

## Token Policy

Known OpenAI model families use `tiktoken-rs` tokenizer counts over the exact
serialized request, including the inline protocol. An applied rewrite must save
more than `max(128 tokens, 3% of the original request)`. Unknown tokenizers and
low-confidence estimates decline rewriting. Provider-observed usage,
tokenizer-counted values, and estimates remain separately labeled.

## Deterministic Evidence

The inputs-only corpus is
[smart_context_replay_corpus.json](../crates/prodex-runtime-proxy/tests/fixtures/smart_context_replay_corpus.json).
It contains 18 scenarios, including a 31-turn continuation, four context-window
sizes, HTTP and WebSocket, exact/shadow/canary/active modes, process restart,
concurrent isolated proxy instances, route rejection, missing artifacts,
build/runtime failures, large diffs, repository navigation, changing
instructions, corrective turns, binary-like output, and duplicate tool output.

Run strict machine-readable evidence:

```bash
npm run smart-context:replay
```

Regenerate or verify checked evidence:

```bash
npm run docs:smart-context-evidence
npm run docs:smart-context-evidence:check
```

The current [raw report](generated/smart-context-replay-report.json) and
[summary](generated/smart-context-replay-report.md) carry their source commit,
toolchain, tokenizer, token counts, and scenario outcomes as generated
provenance. Those figures describe only the checked corpus. They are not a
universal reduction target, latency claim, or live-model quality claim.

Evidence levels are separate:

1. deterministic correctness replay: required in CI;
2. tokenizer/performance benchmarks: deterministic, machine-sensitive, and
   reported with provenance;
3. optional live-model evaluation: non-deterministic and never treated as CI
   proof.

## Migration

New identities use `sc2:`/`psc2:` SHA-256. Scoped schema versions 1 and 2
are validated and rewritten as schema 3 on the next durable save. Legacy
`sc:` references remain read-only compatibility inputs. A root-level
unscoped store cannot prove its security boundary, so Prodex quarantines it
instead of importing it silently.

Rollback and corruption handling follow the common
[rollout and rollback contract](enterprise-governance/22-rollout-rollback-and-deprecation.md).

## Remaining Risks

- Deterministic fixtures prove request invariants, not task quality from a live
  model.
- Allocation per optimized replay turn is captured with the opt-in counting
  allocator. Queue-wait and per-scope lock-wait distributions remain outside
  the current benchmark, so no no-regression claim is made for those waits.
- Persisted-artifact insertion is retained for compatibility and rehydration,
  but active rewrites intentionally emit only self-contained inline references.
