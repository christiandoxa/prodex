# Request Connection token parsing in Mojo (2026-10-23)

Mojo now owns comma splitting, Unicode edge trimming, RFC token acceptance and
ordered token ranges across request Connection fields. The production Rust
parser is deleted. The focused module imports `response_is_tchar` from the
existing response-header owner, keeping one HTTP token grammar. Existing exact
ASCII case-insensitive Connection-name filtering remains compiled in Mojo;
Rust passes only those values, never unrelated authentication header values.

Rust retains header acquisition, borrowed views, checked range allocation and
token materialization. The additive version-1 ABI measures rows before writing
and checks the entire UTF-8 batch, alignment, counts, phases, output capacity and
version first. Rejected input and short capacity leave output and metadata
unchanged. Accepted tokens retain their original casing, order and duplicates.
No new token-count cap changes the public helper's existing behavior; transport
input bounds remain with the HTTP/WebSocket adapters. Invalid Mojo output is an
explicit invariant error, with no Rust parser fallback.

Production callers are the shared HTTP header forwarding path, WebSocket header
forwarding and local rewrite header forwarding. Profile affinity, request auth
replacement, response status/body pass-through and mid-stream rotation rules are
unchanged. Full-object caller assertions preserve session and turn metadata,
including the existing distinction between an exact `Connection` field name
and a padded field name. The runtime hotpath guard passes with no disk IO,
blocking work or synchronization added to the parser.

Verification with `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0`:

- PASS: `rtk cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib runtime_response_header_policy` (4 tests). Differential cases cover the complete ASCII character set, Unicode edge whitespace, invalid tokens, duplicates, empty fields and 20,000 tokens. Raw ABI checks reject malformed later input before writing earlier output.
- PASS: `rtk cargo test --locked -q -p prodex-runtime-proxy` (431 tests).
- PASS: `rtk cargo fmt --check`, `rtk git diff --check`, crate-boundary and runtime-hotpath guards/self-tests, ownership, authority and no-fallback/self-test.
- PASS: optimized objects, the versioned export and absence of Modular runtime references for all six release targets. The context-text compatibility fix also compiles for the five foreign release triples; the Linux production link and 1,112-test Mojo-core suite already passed.
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo test --locked -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1` (459 tests); `rtk proxy npm run test:changed -- --base HEAD --no-untracked`; and the previously started `rtk proxy npm run ci`.
- FOLLOW-UP: the runtime-smoke process later compiled the next uncommitted quota-filter slice and failed on an unqualified `anyhow!` macro. Its namespace is fixed; the suite is repeated with that slice before the campaign handoff.

The RTK checkpoint's provider-constraints-only build, 1,112-test full Mojo-core
suite and final six RTK caller cases have now passed. The repeated default CI completed successfully, including metadata preflight,
Clippy, all 282 Node tests and the selected Rust shards. New quota-filter changes
receive separate focused verification after this earlier run. Earlier failed CI evidence
and the pre-existing Docker scaffold omission remain documented in the preceding
toolchain and RTK reports. Native macOS/Windows final linking and execution,
live providers and release publication remain outside local coverage.

Canonical share is 36.90410217755004% (140,512 Mojo / 240,237 Rust production LOC).
The release floor and non-regression guards pass; the requested 75% campaign
target is still open. Checkpoints through `e1b582adf` are pushed to origin/main.
No release or tag was created. Next deterministic candidate: the quota provider
filter's aliases and normalization, extending the existing quota-watch owner.
