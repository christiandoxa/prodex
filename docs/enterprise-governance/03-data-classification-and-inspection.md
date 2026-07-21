# Data Classification and Inspection

## Status

This document records the implemented Phase 1 and Phase 2 contract. The source
boundaries, policy behavior, and repository evidence are summarized below.
Environment-specific detector capacity and latency certification remains a
deployment acceptance responsibility.

## Control Objectives

Every routed request must have:

- an explicit inspection coverage result;
- an explicit data classification;
- bounded findings and stable reason codes;
- an immutable detector and classification-rule revision;
- deterministic request obligations before provider dispatch;
- deterministic response obligations before or during response commit.

Inspection and classification must not depend on the ingress channel. Channel,
identity, route, session, and requested capabilities are policy attributes, not
bypass mechanisms.

## Classification Model

The production domain model has four ordered levels:

| Level | Intended meaning | Typical examples |
| --- | --- | --- |
| `Public` | Approved for public disclosure | Published documentation and public catalog data |
| `Internal` | Non-public operational information with limited impact if disclosed | Internal procedures and ordinary project metadata |
| `Confidential` | Sensitive business or personal information requiring controlled disclosure | PII, customer records, credentials metadata, and non-public financial information |
| `Restricted` | Highest-sensitivity information requiring the narrowest execution and retention controls | Secrets, private keys, payment/account identifiers, and tenant-defined restricted records |

Classification is monotonic within a request and session context. A later stage
may raise a classification but may not lower it. A trusted explicit label may
raise the automatic result. An untrusted user label may never lower it.
Declassification requires a separate authorized, audited control-plane
operation and is outside ordinary request processing.

`Unknown` may be used only as an internal pre-decision state. It is never a
routeable classification in enforcement modes. Unsupported or unresolved
content remains visible through inspection coverage and policy reason codes;
it must not be silently mapped to `Public`.

Classification inputs are typed and bounded. They may include:

- detector findings;
- a trusted source label;
- route, action, channel, and requested capabilities;
- supported file/media metadata;
- inspection coverage;
- tenant classification rules;
- prompt-injection and tool-execution risk signals;
- prior session/context classification.

The classification result includes the selected level, bounded reason codes,
the classification-rule revision and checksum, and the detector revision. It
does not contain request or response content.

## Inspection Coverage

Coverage describes what was actually inspected, not what the caller requested.

| Coverage | Meaning | Enforcement consequence |
| --- | --- | --- |
| `Full` | Every supported user-content field and modality in the bounded request envelope was inspected by an active detector set | Policy may allow routing subject to classification and obligations |
| `Partial` | Some inspectable content was inspected, but at least one field, segment, or modality was not fully inspected | Policy decides whether to deny, constrain execution, or require an approved local provider |
| `Unsupported` | The request contains no supported inspection representation, or the required detector cannot inspect it safely | Enforcement fails closed unless policy explicitly permits an approved constrained local path |

Detector timeout, malformed output, response overflow, unavailable service,
input truncation, match overflow, and skipped unsupported modalities cannot
produce `Full` coverage.

Binary, image, audio, and video inputs are `Partial` or `Unsupported` unless a
real bounded adapter inspects that modality. Metadata inspection alone is not
full content inspection.

## Domain Contract

Names may follow repository conventions, but the semantics must remain strongly
typed and side-effect-free in `prodex-domain`.

```rust
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

pub enum InspectionCoverage {
    Full,
    Partial,
    Unsupported,
}

pub struct InspectionFinding {
    pub kind: FindingKind,
    pub location: ContentLocation,
    pub confidence_basis_points: u16,
    pub detector_id: DetectorId,
}

pub struct InspectionResult {
    pub coverage: InspectionCoverage,
    pub classification: DataClassification,
    pub findings: BoundedFindings,
    pub tags: BoundedTags,
    pub reason_codes: BoundedReasonCodes,
    pub detector_revision: DetectorRevisionId,
    pub classification_revision: ClassificationRevisionId,
}
```

`InspectionFinding` records only type, bounded structural location, confidence,
and detector identity. It must never retain the matched value. Locations use
bounded field paths and byte/character ranges only where the supported schema
makes them stable. Debug, serialization, error, log, metric, trace, and audit
representations must remain content-free.

Initial finding kinds must cover technically supported forms of:

- common PII such as email, telephone, address, government identifier, and
  person identity;
- access tokens, API keys, private keys, and credentials;
- payment-card and financial-account identifiers;
- tenant-defined sensitive patterns;
- prompt-injection and tool-execution signals as risk findings, not proof of
  malicious intent.

## Bounds

All limits are validated at startup or revision publication and carried in an
immutable snapshot. No request-path regex compilation or unbounded allocation
is permitted.

Required independent bounds include:

- request envelope and inspectable input bytes;
- response inspection bytes and streaming window size;
- JSON nesting depth, collection length, and string length;
- supported content-field count;
- detector count and tenant pattern count;
- compiled pattern size and complexity;
- finding and match count;
- tag and reason-code count and length;
- Presidio request and response bytes;
- per-stage timeout derived from the request deadline;
- external-inspection concurrency and queue capacity;
- stream parser state and incomplete-event buffer;
- classification rule count and compilation complexity.

Crossing a bound produces a stable reason code and non-full coverage. In an
enforcement mode it follows the active failure policy; it is never silently
truncated and reported as fully inspected.

Tenant patterns are compiled during publication or startup with the existing
safe regex boundary. Invalid, duplicate, ambiguous, or excessive patterns
reject the candidate revision while the last-known-good revision remains
active.

## Schema-Aware Content Walker

Inspection parses a bounded request envelope once and walks only documented
user-content locations for the canonical route. It preserves the original
provider protocol structure.

The supported-field registry must cover each enabled request shape explicitly,
including where applicable:

- Responses instructions and input items;
- message/content text parts for chat-completions and Anthropic messages;
- tool and function arguments represented as structured JSON or bounded text;
- supported tool results and uploaded text content;
- compact request content that can contain user data;
- supported WebSocket request events;
- supported embedding input strings.

Unknown fields are preserved but do not become implicitly inspected. Unknown
content-bearing fields, malformed supported fields, binary parts, and
unsupported modalities lower coverage. The walker must not stringify an entire
arbitrary JSON body and regex-replace it. It must not join unrelated values with
a sentinel and assume the detector preserved the sentinel count.

Each route/schema adapter has characterization tests proving which paths are
inspectable and which produce `Partial` or `Unsupported` coverage. Aliases and
compatibility routes resolve to the same canonical schema adapter.

## Detector Composition

The application inspection use case combines detector outputs without making
an adapter the policy owner:

```text
bounded schema walk
-> bounded local secret and tenant-pattern detection
-> optional approved Presidio analysis
-> normalize and validate findings
-> deduplicate and cap findings
-> determine coverage
-> classify using the immutable rule snapshot
-> return a typed inspection result
```

Local detectors provide deterministic coverage for secret-like material and
approved tenant patterns. Presidio remains an external detector/anonymizer
adapter in `prodex-presidio`; it does not decide classification, provider
eligibility, failure mode, or audit policy.

Detector results are validated before use. Ranges must be ordered, in bounds,
and aligned to the inspected representation. Confidence is normalized to
integer basis points. Unknown entity kinds, invalid ranges, excessive findings,
and malformed responses lower coverage or fail the stage according to mode.

## Presidio Trust Boundary

External inspection receives raw content and therefore requires a stricter
trust decision than an ordinary telemetry sink.

- Personal mode may use explicit local configuration with clear operator
  ownership.
- Enterprise and bank modes accept only explicitly approved trusted or on-prem
  endpoints through the hardened endpoint and egress policy.
- Endpoint configuration rejects credentials, query strings, fragments,
  ambiguous hosts, disallowed schemes, unapproved origins, and unsafe redirects.
- DNS resolution and connection policy must prevent SSRF and rebinding outside
  the approved destination set.
- TLS verification is mandatory for non-loopback HTTPS endpoints. mTLS may be
  required by deployment policy.
- Authentication uses `SecretRef`; raw tokens are absent from policy, URLs,
  arguments, logs, and health output.
- The client uses bounded timeouts, response bytes, concurrency, redirects, and
  connection pools.
- Raw content is never sent to an unapproved third-party inspection service.

Health checks and detector revision refresh occur in bounded background work.
The request path may call Presidio only when the active inspection policy
requires it and within the request deadline.

## Masking and Structure Preservation

Masking is a typed policy obligation applied after inspection and before
provider dispatch. A finding alone does not authorize a transformation.

Placeholders are deterministic by finding kind within one request and do not
contain the source value. A suitable shape is `<PRODEX:EMAIL:1>`, with the
sequence scoped to the request. Placeholders must not become stable
cross-request identifiers.

Masking rules:

- mutate only supported content segments selected by the obligation;
- apply replacements in a deterministic non-overlapping order;
- preserve valid JSON and the provider request schema;
- preserve tool/function argument structure when it is parsed as JSON;
- reject rather than mask if transformation would make the request ambiguous,
  corrupt protocol semantics, or leave unacceptable residual risk;
- record finding kinds, counts, action, and reason codes without content;
- zero or drop short-lived raw intermediate buffers when practical.

The response path uses the same finding semantics and separate typed response
obligations. Request masking does not imply that response inspection is
optional.

## Streaming Response Behavior

Unary responses are inspected before they are returned when response
inspection is required.

Streaming inspection uses a bounded incremental parser and sliding window that
can detect supported findings across transport chunk boundaries. It operates on
normalized provider events rather than arbitrary transport fragments whenever
the provider adapter exposes structured events.

- Before the first local commit, a violation returns a stable redacted local
  denial.
- After output is committed, a violation terminates the stream safely, stops
  further content delivery, reconciles usage and cancellation, and emits an
  exact content-free audit reason.
- It must not retry, rotate, or synthesize a misleading upstream error after
  commit.
- Incomplete or malformed provider events lower coverage or terminate according
  to the active response obligation and deployment mode.
- Tool calls and supported tool results are inspected according to policy.

The state machine bounds buffered bytes, UTF-8 carry state, event depth,
finding count, and tail/window length. Tests split every supported finding at
every relevant chunk boundary.

## Failure Modes

Deployment mode owns failure behavior. Detector adapters return typed outcomes;
they do not choose fail-open behavior.

| Deployment mode | Inspection rollout | Timeout, unavailable, malformed, overflow, or unsupported required content |
| --- | --- | --- |
| `personal` | `off`, `observe`, or explicitly enabled `enforce` | Backward-compatible behavior is allowed when governance is off; enabled enforcement follows its explicit policy |
| `enterprise_observe` | Shadow decision only | Preserve legacy-compatible routing, emit bounded content-free evidence, and never report false `Full` coverage |
| `enterprise_enforce` | Enforced per tenant revision | Deny or constrain to an explicitly eligible approved provider according to policy; no implicit cloud fallback |
| `bank_enforce` | Mandatory enforce | Fail closed for required inspection, unresolved classification, stale/invalid mandatory snapshots, or unsupported sensitive content, except an explicitly approved local constrained path |

Observe mode never weakens an existing denial, authentication check, provider
constraint, or secret rule. Feature flags cannot disable mandatory bank-mode
inspection. Kill switches select the last-known-good approved revision or stop
affected traffic; they do not bypass the control.

## Audit and Observability

Each stage emits low-cardinality metrics for duration, coverage, finding
category, masking action, timeout, error, and denied outcome. Labels exclude raw
tenant/user identifiers, filenames, field paths, exact locations, detector
messages, and policy text.

Mandatory audit metadata includes request/trace correlation, pseudonymous
principal reference, canonical action and route, coverage, classification,
finding-kind counts, detector and classification revisions, obligation action,
outcome, and stable reason codes. It excludes prompts, responses, tool
arguments, uploaded bodies, raw matches, arbitrary headers, tokens, and full IP
addresses.

## Phase 1 Migration

Phase 1 establishes one request inspection boundary without changing transport
semantics.

1. Add the bounded pure inspection types and validation tests to
   `prodex-domain`.
2. Add one `prodex-application` request-inspection use case that accepts the
   canonical schema walk and normalized detector outputs.
3. Adapt existing local secret/PII detection and `prodex-presidio` to the typed
   result. Reuse the existing endpoint and secret-reference hardening.
4. Wire the use case after bounded body capture and before reservation/provider
   dispatch for one canonical route, starting with Responses HTTP.
5. Remove the second authoritative PII-redaction call for that migrated route.
6. Preserve personal/off behavior and add explicit observe/enforce outcomes.
7. Expand the same boundary to compact, messages, chat completions, embeddings,
   tool content, and WebSocket events. Do not create route-local policy copies.
8. Add inspection stage metrics and mandatory content-free denial audit.

Phase 1 exits only when every supported request path either uses this boundary
or returns explicit unsupported coverage to the same policy path, and duplicate
PII-removal policy has no production callers.

## Phase 2 Migration

Phase 2 adds classification and bidirectional enforcement to the Phase 1
boundary.

1. Add the four-level classification engine, monotonic merge, bounded reason
   codes, and revisioned compiled rule snapshot.
2. Add trusted-label raising and keep classification lowering unavailable; any
   future declassification workflow requires a separately versioned approval,
   audit, and threat-model contract.
3. Carry inspection summary, coverage, classification, and revisions in the
   immutable governed request context.
4. Persist only minimum session classification and revision metadata; bind it
   to tenant, principal, channel policy, and credential scope.
5. Add typed request masking/deny obligations and response inspection
   obligations.
6. Add unary and incremental SSE/WebSocket response inspection with correct
   pre-commit and post-commit behavior.
7. Enable tenant-scoped `off -> observe -> enforce` rollout using immutable
   last-known-good snapshots and audited rollback.

Phase 2 exits only when every routed request has an explicit classification and
coverage and every supported response path has tested enforcement behavior.

## Required Verification

### Unit and table-driven tests

- all four classifications and monotonic merge;
- `Full`, `Partial`, and `Unsupported` coverage;
- every supported request content path and unsupported modality;
- deterministic placeholders, overlap resolution, and JSON preservation;
- local secret, credential, PII, payment/account, and tenant-pattern findings;
- trusted label raising and untrusted lowering rejection;
- rule and detector revision validation;
- failure-mode matrix for every deployment and rollout mode.

### Adversarial and property tests

- Unicode normalization, confusables, combining characters, and invalid UTF-8;
- findings split across JSON strings and stream chunks;
- deep/nested objects, huge strings, field floods, match floods, and response
  overflow;
- malformed schemas, provider events, detector ranges, confidence values, and
  entity kinds;
- regex complexity and excessive tenant patterns;
- masking always preserves valid supported structure;
- classification is deterministic and never decreases;
- logs, traces, audit, errors, and health output never contain source content or
  raw matches.

### Adapter and integration tests

- Presidio timeout, refusal, redirect, malformed JSON, invalid range, excessive
  response, DNS/SSRF, and unapproved-origin cases;
- bounded external-inspection concurrency and queue saturation;
- unary, SSE, and WebSocket request/response paths;
- local and remote providers, tools on/off, and compatibility aliases;
- off-mode compatibility and observe-mode non-authoritative decisions;
- enforce and bank-mode fail-closed behavior;
- cancellation and usage reconciliation after a committed stream is stopped.

### Performance evidence

Benchmarks must cover no findings, many findings, maximum allowed input,
Unicode, structured masking, stream boundary detection, snapshot refresh, and
external-detector saturation. External Presidio latency is reported separately
from local classification and policy latency.

## Current Repository State

The repository implements the source-owned controls in this contract:

- `prodex-domain` owns bounded `InspectionResult`, coverage, four-level
  classification, content-free findings, reason codes, and revision types;
- `prodex-application` owns inspection, classification, policy, and obligation
  decisions independently of transport adapters;
- runtime request paths use schema-aware local/Presidio inspection and carry
  explicit coverage into governance decisions; unsupported content remains
  explicit and fails according to deployment mode;
- response paths provide unary and bounded incremental enforcement, including
  pre-commit denial and post-commit termination without retry or rotation;
- Gemini Live and WebSocket paths enter governance before provider dispatch and
  apply bounded incremental response inspection;
- Presidio is opt-in, bounded, and subject to mode-specific trust and egress
  validation; its findings are normalized into the typed inspection boundary;
- immutable policy/classification revisions, tenant rollout modes, last-known-
  good activation, and content-free audit evidence are wired through the
  gateway runtime;
- the machine-readable security matrix, domain/application/runtime integration
  tests, and `benches/governance_hot_paths.rs` provide repository evidence.

Production Presidio saturation, network latency, and organization-specific
unsupported-modality adapters must still be certified in the target deployment.
That external acceptance work does not change the fail-closed source behavior
or create an unimplemented source-code bypass.
