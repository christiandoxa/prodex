# Testing, Performance, and Evidence

## Evidence Rule

A requirement is complete only when its production path, negative behavior,
and current reproducible evidence agree. A type, planner, fixture, document,
mock, or historical benchmark is useful evidence but is not by itself proof of
an active production control.

Never report a command as passed unless it was executed. Record environmental
blockers, skipped tests, failed attempts, variance, unsupported measurements,
and known baseline failures without relabeling them as success. Security,
tenant isolation, mandatory audit, bounded resources, streaming commitment,
affinity, and accounting correctness override performance results.

## Current Repository Evidence

Reproducible evidence is produced by the checked-in CI, benchmark, load,
storage-proof, backup/restore, and documentation commands. Generated Smart
Context reports carry their own source/toolchain provenance. `package-lock.json`
is present and `npm ci` is part of the reproducible workflow.

Measurements apply only to the named path and exact recorded environment.
Inspection, PDP, approval, governed routing, session, SIEM, secret-provider,
and bank-mode controls use their own focused evidence; unrelated Smart Context
or CLI measurements cannot be reused as proof.

## Required Test Matrix

The machine-readable matrix must assign a stable ID to each threat and control,
then map it to the production mode, phase, test command, expected outcome,
evidence artifact, and owner. At minimum it covers the following layers.

`test-matrix.json` uses schema version 2. Every `tests` row must contain:

- `id`: unique stable threat/control identifier;
- `phase`: positive phase number;
- `modes`: one or more production modes such as `enterprise_enforce` or
  `bank_enforce`;
- `test_command`: one stable checked-in `cargo test`, `npm run`, or guard command
  that can be rerun without inventing a test; `planned:` and `external:` commands
  are allowed only for rows whose status is not `tested` or `implemented`;
- `expected`: expected control outcome;
- `evidence_artifact`: run-independent locator for the immutable evidence bundle;
- `owner`: accountable repository or deployment owner;
- `evidence`: non-empty evidence references or descriptions.

Identifier-shaped `evidence` entries are ordinary repository test references and
must resolve to a checked-in test declaration. Guard filenames must resolve under
`scripts/ci/`. Commands and deployment claims that are not repository tests stay
explicitly marked as `external:` descriptions; they are not silently counted as
automated test evidence. Missing fields and stale ordinary references fail the
enterprise-docs guard deterministically.

### Unit and golden tests

- classification ordering, monotonicity, coverage, and bounded findings;
- Unicode-aware detector locations and masking that preserves valid schemas;
- policy input, explicit-deny precedence, obligation merge/conflict, stable
  reason codes, and deterministic evaluation;
- approval state transitions, quorum, expiry, idempotency, maker-checker, and
  self-approval denial;
- provider hard eligibility, fixed-point scoring, deterministic tie-break,
  bounded score explanation, and ineligible-set denial;
- session binding, absolute/idle expiry, revocation, replay, concurrency,
  classification and revision pinning;
- audit canonicalization, append chain, tamper/gap detection, outbox delivery
  IDs, and mandatory failure outcomes;
- typed configuration, insecure bank-combination rejection, and stable redacted
  errors.

### Property and fuzz tests

- bounded JSON/content walking across depth, value, byte, detector, finding,
  and media limits;
- masking preserves JSON/tool argument structure and cannot expose a finding
  through field ordering or Unicode boundaries;
- Unicode normalization, confusables, invalid encodings, and offset conversion;
- response inspection across every relevant stream chunk boundary;
- bounded policy parsing/compilation, rule limits, ambiguity, contradictions,
  cycles, and safe pattern behavior;
- obligation merge never weakens an explicit deny or mandatory restriction;
- routing never selects outside the original hard eligible set and produces the
  same result for identical immutable input;
- audit serialization round trips and any mutation fails verification;
- session, token metadata, canonical request-target, header, and cursor parsing.

Fuzz corpora and failure artifacts must contain only synthetic, non-sensitive
data and remain size bounded.

### Integration and contract tests

- CLI, IDE, browser/API, service, compact, SSE, and WebSocket channels reach the
  same governed application use case;
- authn -> authz -> session -> inspection/classification -> PDP -> admission ->
  routing -> provider -> response guard -> reconciliation/audit order;
- SQLite local behavior and PostgreSQL authoritative/RLS behavior;
- Redis loss and invalidation without authoritative-state loss;
- Presidio endpoint, timeout, concurrency, response-bound, and failure-mode
  behavior;
- Vault/secret-provider lease, renewal, rotation, expiry, and outage;
- SIEM durable outbox, retry, idempotency, backlog, dead letter, and recovery;
- provider SPI conformance and capability-gated unsupported operations;
- OpenAPI, error-envelope, CLI, Codex header/affinity, and API-version
  compatibility;
- multi-replica activation, revocation, rate limit, drain, and restart.

### Security and adversarial tests

- cross-tenant access, confused deputy, credential-scope confusion, and tenant
  inference through errors, idempotency, counts, or unique constraints;
- stale/self/replayed approval, quorum bypass, break-glass overreach, and
  activation TOCTOU;
- PII, credentials, private keys, financial identifiers, and exfiltration data
  split across fields, tool arguments, stream chunks, and Unicode forms;
- detector/policy complexity attacks, deep/oversized/malformed requests, and
  finding floods;
- spoofed forwarding, client certificate headers, IP/geo risk, Host, Origin,
  CSRF, request smuggling, and canonical-target discrepancies;
- stale policy/registry, provider revocation, ineligible fallback, generic 429,
  and mid-stream failure behavior;
- SSRF and endpoint abuse against IdP/JWKS, Presidio, provider, Vault, SIEM, and
  webhooks;
- prompt/tool injection signals, unsupported media, raw-content leakage in
  logs/metrics/traces/audit/errors/debug/health, and secret leakage;
- audit tamper/gap, DNS/egress bypass, denial of service, and bounded-resource
  assertions;
- full `bank_enforce` fail-closed scenarios for identity, classification,
  policy, registry, secrets, accounting, audit, and storage.

### Reliability and chaos tests

- slow client, slow upstream, slow external inspection, and bounded queues;
- provider outage before and after stream commit, partial streams, and
  cancellation;
- Redis total loss, stale replica, eviction, and recovery;
- PostgreSQL failover, unavailability, unknown commit, pool exhaustion, and
  connection tenant-context reset;
- Vault lease expiry and SIEM backlog/dead-letter recovery;
- policy/registry refresh races, control-plane outage, and LKG expiry;
- process restart during activation, approval consumption, audit append, or
  outbox delivery;
- graceful drain with unary and active streams;
- backup/PITR restore, policy/registry consistency, RLS, audit-chain, outbox,
  retention, and legal-hold verification.

## Governance Benchmark Program

### Required cases

| Benchmark family | Representative cases | Maximum bounded cases |
| --- | --- | --- |
| Inspection | No findings, common findings, Unicode, nested tool arguments, supported media, partial coverage | Maximum input bytes/depth/values/detectors/findings; timeout and concurrency saturation measured separately |
| Classification | Trusted-label and finding merge, session monotonicity, obligation classification merge | Maximum rule and tag/reason counts |
| PDP | Common allow, explicit deny, conflicting obligations, high-risk approval | Maximum supported compiled rules, attributes, obligations, and reason count |
| Routing | Typical eligible pool, affinity, quota/health changes, compliant fallback | Maximum registry/candidate count with worst-case hard filters and fixed-point score components |
| Sessions | Lookup, touch, expiry, revocation, concurrency admission, invalidation | Maximum supported session/cache pressure and multi-replica revocation fanout |
| Audit/SIEM | Append, chain verify, outbox enqueue/export/ack | Maximum bounded batch, exporter outage, backlog pressure, dead-letter transition |
| Snapshot refresh | Policy/classification/registry/pricing publish and read under load | Maximum bounded revision/rule/provider set while data plane remains active |
| Provider reliability | Circuit transition, pre-commit retry, eligible fallback, reconciliation | Contended candidate/endpoint set and simultaneous failure signals |
| End to end | Unary, SSE, WebSocket, compact where supported in each governance mode | Reference peak load, slow client/upstream, cancellation, saturation, and drain |

End-to-end measurements cover `off`, `observe`, `enterprise_enforce`, and
`bank_enforce` semantics corresponding to the final typed configuration names.
No benchmark may disable a mandatory control to obtain a better result.

### Acceptance budgets

Unless a measured baseline justifies a reviewed alternative, the program uses:

- governance disabled: no more than 5% p95/p99 latency or throughput regression
  versus the accepted comparable baseline;
- in-memory classification, PDP, obligation merge, and routing: no more than
  5 ms p99 added latency at documented reference load and maximum supported
  bounded configuration, excluding external detector/provider latency;
- policy decision alone: no more than 1 ms p99 for the documented
  representative policy set;
- routing decision alone: no more than 2 ms p99 for the documented
  representative provider set;
- no material CPU/request, allocation/request, RSS, queue-wait, or lock-wait
  regression without an explicit reviewed security tradeoff;
- external inspection timeout, concurrency, latency, and failure rate measured
  separately from in-memory governance;
- at least five comparable samples with hardware, runtime, build, and variance
  metadata.

A test, isolation, audit, backpressure, affinity, or security regression is a
release failure even when every latency budget passes.

## Sampling Method

For every before/after performance comparison:

1. Pin exact source revision, dependency lock, feature set, build profile,
   configuration revisions, policy/registry fixtures, and synthetic workload.
2. Record OS/kernel, CPU model/topology, governor/frequency/boost state, RAM,
   Rust/LLVM, allocator, container limits, database/Redis/provider topology,
   and clock source.
3. Build and warm once. Stop unrelated builds and load generators.
4. Run at least five independent samples in randomized or interleaved order
   when practical, with identical commands and clean state.
5. Preserve raw harness output. Report sample values, median, p50/p95/p99 when
   available, throughput, error/denial counts, CPU/request, RSS,
   allocation/request, queue wait, and lock wait.
6. Mark unsupported measurements as unsupported. Do not infer allocation from
   RSS, queue wait from pressure markers, or RPO from dump duration.
7. Compare variance and confidence before applying the 5% guard. Investigate a
   threshold miss; do not delete or retune the case after seeing the result.
8. Run correctness/security gates on the exact candidate binary and fixtures
   used for the accepted measurements.

External provider and detector latency must use controlled synthetic endpoints
for reproducibility. Live-provider observations, if collected, are a separate
operational study and must not contain customer data or credentials in the
evidence bundle.

## Evidence Bundle Format

Each gate or benchmark run produces an immutable directory or artifact with:

```text
evidence/<program>/<run-id>/
  manifest.json
  commands.jsonl
  environment.json
  configuration-digests.json
  results.json
  raw/
  logs-redacted/
  attestations/
```

The bundle must not contain raw prompt/response content, tokens, secret values,
full IP addresses, production tenant/user/provider names, or credential-bearing
URLs.

### Manifest fields

The minimum machine-readable manifest is:

```json
{
  "schema_version": 1,
  "program": "governance-benchmark",
  "run_id": "synthetic-run-0001",
  "started_at": "2026-07-13T00:00:00Z",
  "source_revision": "synthetic-revision",
  "dirty_worktree": false,
  "build_profile": "release",
  "governance_mode": "enterprise_enforce",
  "fixture_revision": "synthetic-fixture-v1",
  "sample_count": 5,
  "result": "blocked",
  "blockers": ["example dependency unavailable"],
  "artifacts": [
    {
      "path": "results.json",
      "sha256": "synthetic-sha256"
    }
  ]
}
```

Use a real digest in generated evidence. The values above are intentionally
synthetic documentation examples.

### Command record

Each command record contains the exact argv represented as a safe structured
array, working directory class, start/end time, exit code or signal, duration,
stdout/stderr artifact digests, redaction version, and operator/automation
identity reference. Do not store secret-bearing environment or reconstruct a
shell command that embeds credentials.

### Result record

Each benchmark result identifies case, unit, mode, bounds, sample values,
summary statistics, variance, before/after revisions, delta, threshold,
pass/fail/blocked disposition, and interpretation. Each security/control test
identifies threat/control IDs, expected and observed outcomes, mode, phase,
test path, artifact digests, and disposition.

Artifacts must be written atomically, integrity protected, retained according
to evidence policy, and stored outside the system being tested for disaster-
recovery evidence. Any manual waiver records approver, scope, expiry, rationale,
compensating control, and follow-up owner.

## Exact Quality Commands

The final candidate runs, at minimum, the applicable repository commands below.
The list is a required program; pass/fail evidence comes from the actual run.

```bash
cargo fmt --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
npm run test:full -- --timings
npm ci
npm test
npm run docs:lint
npm run ci:crate-boundary
npm run ci:domain-boundary-guard
npm run ci:application-boundary-guard
npm run ci:auth-boundary-guard
npm run ci:gateway-core-boundary-guard
npm run ci:gateway-http-boundary-guard
npm run ci:deployment-security-guard
npm run ci:dependency-duplicates
cargo audit
cargo deny check advisories sources licenses bans
```

Also execute the applicable repository fuzz, migration, PostgreSQL proof,
Redis/SQLite boundary, backup/restore, multi-replica, load, stress, and
benchmark commands. Existing local performance entry points include:

```bash
cargo bench --locked --bench governance_hot_paths
cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths
PRODEX_RUNTIME_PROXY_BENCH_CHECK=1 \
  cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths
npm run load:self-test
npm run load:runtime-proxy
npm run ci:runtime-load-smoke
npm run ci:runtime-stress
npm run ci:backup-restore-drill
npm run ci:storage-postgres-proof
```

## Release Evidence Gate

A phase exits only when:

- production wiring and negative tests cover every required channel and mode;
- the machine-readable threat/control matrix has no unexplained missing test;
- all mandatory CI and architecture guards pass, or an exact environmental
  blocker is recorded without weakening the candidate;
- five-sample before/after evidence covers the metrics the phase can affect;
- all unsupported measurements and failed cases remain visible;
- deployment, migration, backup/restore, and chaos evidence uses the actual
  candidate schema and governance entities;
- the ledger links the source revision, exact commands, results, artifact
  digests, blockers, owner, and next action.

No result in this document should be interpreted as final Phase 5 evidence
until the full program is executed against the completed governed production
path.
