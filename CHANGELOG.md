# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.6 - 2026-10-12

### Runtime

- Keep large-session recovery armed after compaction (`7151012`)

### Deps

- Bump rust from 1.98.1-bookworm to 1.99.0-bookworm (`db5cb81`)
- Bump the cargo group with 6 updates (`ea659d2`)

### Misc

- Use Rust 1.99 atomic update API (`c757a6a`)
- Merge Dependabot PR #111 for 0.437.6 (`4ac9273`)
- Merge Dependabot PR #110 for 0.437.6 (`02eb770`)
- Merge Dependabot PR #109 for 0.437.6 (`0164486`)
# Prodex 0.437.6

## New Features

No new user-facing features. This patch restores automatic recovery for large
persisted sessions and qualifies the Rust 1.99.0 toolchain.

## Bug Fixes

### Keep recovery active on long-lived sessions

The recovery monitor previously obtained a JSONL cursor by loading the entire
rollout through a reader limited to 64 MiB. Large sessions could therefore start
monitoring at offset zero instead of the current end. The next incremental scan
then rejected the oversized historical tail and permanently disabled workflow
monitoring. A native `task_complete` event carrying `server_overloaded` could be
missed even when another profile was eligible for rotation. This was especially
visible when a resumed task compacted its history and then encountered capacity
pressure, leaving the goal stalled until another manual resume.

Plain JSONL cursor acquisition now uses metadata from the validated open regular
file, without loading historical content. Full-document reads retain their
allocation limit; incremental tail limits, symlink rejection, path identity
checks, complete-record boundaries, and compressed decoded offsets remain intact.
Transient scan failures retain their cursor and evidence and retry after a bounded
five-second delay rather than muting recovery for the rest of the process.

Recovery continues the persisted session and goal. It does not resubmit the
original prompt or repeat completed tool calls. Cancellation, explicit rotation
opt-out, model selection, and existing ownership safeguards remain in effect.

## Dependencies and Toolchain

Rust 1.99.0 is pinned consistently in the workspace, toolchain configuration, CI,
release jobs, and digest-pinned Docker builder. Atomic compare/update callers use
the renamed `try_update` API without changing their ordering or closure behavior.

This release incorporates Dependabot PRs #109, #110, and #111: the Cargo dependency
group, the SHA-pinned GitHub Actions group, and the Rust Docker image update.
The Cargo update includes Hyper, libc, Tokio, TOML, UUID, and zeroize together with
the lockfile's related transitive updates. Mojo remains at 1.1.0.

## Verification

The original implementation fails a regression that obtains a large rollout's
cursor through the production length API, instead of injecting a known offset.
A production-monitor regression then verifies a rollout larger than 64 MiB,
compaction, a native capacity failure, and selection of the next profile without
replaying historical failures. A second regression covers a transient session
read failure, bounded retry, and subsequent recovery.

Both regressions reject deliberate reintroductions of their corresponding bugs.
The mutated sources are restored byte-for-byte and the caller tests pass again.
Existing compressed-session, cancellation, rotation opt-out, completed-work,
foreign-session, and unaccepted-turn regressions remain required.

## Upgrade Notes

Start a new Prodex process after installing this release. A supervisor already
running an older executable keeps its old monitoring state; replacing the binary
on disk does not change that process. Preserve unsent drafts and finish or safely
interrupt active work before relaunching and resuming the saved session.

Available account quota does not guarantee upstream capacity for a particular
model. This patch repairs local recovery observation and bounded retries; it does
not promise that an unavailable upstream model will always accept a request.

## Changelog

- Obtain large JSONL recovery cursors without loading historical session content.
- Retry transient session scans without losing recovery evidence or position.
- Verify compaction-to-capacity recovery and reject deliberate bug reintroductions.
- Adopt Rust 1.99.0 and its atomic update API with consistent CI and Docker pins.
- Include Dependabot Cargo, GitHub Actions, and Docker updates from #109 to #111.
- Retain the Mojo-owned runtime fault-budget transition checkpoint.

Full Changelog: [0.437.5...0.437.6](https://github.com/christiandoxa/prodex/compare/0.437.5...0.437.6)

## 0.437.5 - 2026-10-11

### Runtime

- Stop wrapped native TUI before retiring its endpoint (`1fe2262`)

### CLI

- Preserve long activation profile names (`386db91`)

### Misc

- Repair native goal attachment aliases before resume (`494c414`)

## 0.437.4 - 2026-10-11

### Runtime

- Reset recovery evidence on native task start (`886bcc5`)
- Recover native Codex task completion failures (`96cf9ab`)

## 0.437.3 - 2026-10-10

### Runtime

- Reap the session-owned native app-server process tree (`2efed0b`)
- Prevent admission and quota-state notification deadlock (`370ee38`)

## 0.437.2 - 2026-10-10

### Runtime

- Preserve portable affinity recovery and release validation (`a5b927b`)
- Keep quota recovery reachable across busy accounts (`57a1ee2`)
- Move compact continuation policy to Mojo (`5f91526`)
- Exhaust websocket attempts correctly across recovery sweeps (`d191200`)
- Map Mojo affinity policy errors at app boundary (`d7cc909`)
- Migrate runtime proxy body limit policy (`5357e12`)

### CLI

- Preserve turn-state owner facts in quota replay eligibility (`77ffdf0`)

### Docs

- Qualify Codex rust-v0.162.1 (`d59ab48`)

### Misc

- Preserve DeepSeek usage output and metadata edge cases (`f71e135`)
- Preserve Gemini data URL and media text contracts (`0b84d7a`)
- Select oldest cookie across bounded ABI chunks without Rust fallback (`fff216f`)

## 0.437.1 - 2026-10-09

### Runtime

- Preserve previous-response recovery precedence (`50f0004`)
- Recover headerless and bound-turn upstream capacity (`bcecbb6`)
- Move summary log formatting to Mojo (`e600780`)

### CLI

- Saturate profile geometry at integer boundaries (`75365cc`)
- Migrate profile screen geometry policy (`9e04540`)

## 0.437.0 - 2026-10-09

### Runtime

- Retain streamed rate-limit retry advice in Mojo (`c0f3b1b`)
- Expose shared JSON ABI to runtime-only builds (`c9ee979`)
- Recover delayed SSE capacity failures without retaining permits (`bba3a25`)
- Keep queued fanout out of upstream retry budgets (`cfa0681`)
- Wait through cold-start probe slices (`378ca83`)
- Recover post-compaction quota affinity (`5f5eb4d`)
- Satisfy compact recovery clippy (`466175b`)
- Close 0.435.6 CI regressions (`c0644d6`)
- Keep usable profiles transparent (`ba72da5`)
- Replay hard-affinity quota blocks (`6515c54`)
- Honor request-local quota exclusions (`4f3e21a`)
- Reselect profiles after recovery wait (`12d69a1`)
- Keep retryable profiles alive past precommit budget (`0934e00`)
- Backpressure local saturation (`8aaf4e9`)
- Stop exhausted same-request probe races (`063f81f`)
- Keep auto-rotate alive while quota remains (`02997e4`)
- Migrate websocket recovery policy (`a65206e`)
- Migrate runtime health backoff policy (`b54ee48`)
- Migrate websocket response failure policy (`068a5ed`)
- Migrate websocket failure policy (`a4da101`)
- Migrate runtime log retention policy (`6e42ebe`)
- Migrate runtime admission policy (`d0832e6`)

### CLI

- Restore profile management capacity guard (`2f58db6`)
- Migrate concurrency choices to Mojo (`822eaf7`)
- Delegate profile health circuit timing (`a00b4dc`)
- Prompt expose launch configuration (`ba35576`)
- Migrate profile password policy (`10cb1c1`)
- Expose validated optional tools to exec (`fe31ee4`)
- Migrate profile identity source policy (`6aa569f`)
- Migrate quota auth policy (`303ac12`)
- Migrate profile import identity policy (`11ede5e`)
- Migrate profile backoff policy (`57ebb15`)
- Migrate profile score policy (`3920a9b`)
- Migrate cached quota source policy (`5589543`)

### Claude

- Migrate Anthropic retry transition (`3f7b187`)

### Docs

- Record compact and sub-agent ownership (`1a2197a`)
- Record runtime-doctor Mojo proof (`61fc70d`)
- Record integrated Mojo migration validation (`6150fa1`)
- Record compact-exit checkpoint (`6b67cd6`)
- Record sub-agent renderer wave (`c5af517`)
- Record provider surface migration checkpoint (`4e175fc`)
- Add 0.435.1 release notes (`9c6bc5d`)
- Complete 0.435.0 release notes (`fc41473`)
- Dedupe provider bridge checkpoint (`3b38d70`)
- Dedupe Mojo campaign checkpoint (`aec02ee`)

### Deps

- Qualify Kiro CLI 2.28.0 (`0a55345`)
- Qualify tunnel-client 0.0.16 (`131c41c`)
- Bump tokio-rustls from 0.26.5 to 0.26.6 in the cargo group (`d122561`)

### Misc

- Point scalar reachability to extracted ABI (`1d6c8d1`)
- Preserve long shell arguments in Mojo parser (`5b81c6b`)
- Accept large app-server pages through Mojo ABI (`51f66d7`)
- Preserve Codex 0.161 program arguments in Mojo (`eac2f58`)
- Add OpenAI model effort picker (`6842e68`)
- Preserve large response metadata (`5ca2916`)
- Merge pull request #101 from christiandoxa/campaign/mojo-capsule-order-split-20261004 (`07a1995`)
- Merge pull request #104 from christiandoxa/campaign/mojo-wave-observability-20261004 (`bc0690f`)
- Merge pull request #108 from christiandoxa/dependabot/github_actions/github-actions-6de4297576 (`3c12233`)
- Merge pull request #107 from christiandoxa/dependabot/cargo/cargo-bbe38bb5bb (`c3f5edb`)
- Merge pull request #105 from christiandoxa/campaign/mojo-local-rewrite-policy-20261004 (`d1d7a32`)
- Merge pull request #106 from christiandoxa/campaign/mojo-log-load-aggregate-20261004 (`2556f62`)
- Merge pull request #102 from christiandoxa/campaign/mojo-wave-config-20261004 (`f546cec`)
- Merge pull request #103 from christiandoxa/campaign/mojo-wave-policy-20261004 (`69d079b`)
- Align Mojo fallback self tests (`99c68d8`)
- Align provider defaults with latest models (`ecacd64`)
- Retain active Copilot GPT-5.6 models (`b2e0611`)
- Sync latest provider models (`e6965ee`)
- Align Codex 0.160.0 (`b3ae01d`)
- Qualify Caveman 3.0.0 (`827f048`)
- Classify private key fields as sensitive (`a08813b`)
- Align Codex 0.159.3 (`4a6f160`)
- Migrate noncompact failure policy (`5d06911`)
- Migrate Super choice policy (`1a93be4`)
- Migrate login menu policy (`92c61df`)
- Migrate lineage lookup policy (`3bd28ea`)
- Migrate repo map indexing policy (`2f3dbed`)
- Migrate login argument policy (`50d6191`)
- Migrate provider SSE framing policy (`8d3d1a1`)
- Migrate Presidio transport policy (`baf86b6`)
- Migrate Super prompt step policy (`697a27c`)
- Migrate waitable candidate policy (`3a16ee9`)
- Migrate response usage progress policy (`7b71251`)
- Migrate Gemini SSE lifecycle policy (`90ae761`)
- Migrate hard binding resolution (`29c24ba`)
- Migrate upstream payload classification (`c51bc72`)
- Migrate lineage release policy (`a6cf981`)
- Migrate Presidio JSON field policy (`3319ebe`)
- Migrate provider route parsing (`a5b37b8`)
- Migrate Kiro response input policy (`f4763b8`)
- Migrate provider retry policy (`06622ce`)
- Migrate affinity outcome policy (`46b7c6c`)
- Migrate DeepSeek native search policy (`ea15989`)
- Migrate Gemini unified diff parser (`4ec4ed9`)

## 0.434.2 - 2026-09-30

### CLI

- Auto-trust local workspaces (`21ead45`)

## 0.434.1 - 2026-09-30

### Claude

- Migrate Kiro Anthropic response (`2bb110c`)

### Misc

- Align Codex 0.159.2 (`016a520`)
- Migrate Kiro chat message items (`9cc96a9`)
- Align Codex 0.159.1 (`0fa6400`)
- Migrate Copilot request policy (`a4d3c39`)
- Migrate DeepSeek request planning (`7aa2bd5`)
- Migrate Gemini request contents (`034b6c1`)
- Migrate Gemini Code Assist policy (`5d75057`)

## 0.434.0 - 2026-09-29

### Runtime

- Migrate runtime cookie identity (`9b6755b`)
- Migrate runtime scalar config (`4376b2b`)
- Remove runtime proxy identity duplicates (`b36c6c3`)
- Migrate runtime proxy casefold checks (`d69220f`)
- Remove runtime header identity duplicates (`c9df55c`)
- Migrate OpenAI runtime model policy (`bdbaefe`)
- Remove runtime boolean duplicates (`7f1bf29`)
- Centralize runtime catalog identity (`905fc58`)
- Centralize runtime provider classification (`9109253`)

### CLI

- Support mixed attachment separators (`a37b9c8`)
- Persist resumed pasted attachments (`670e153`)
- Migrate quota render casefold policy (`97a9e27`)
- Remember thread model and trust workspace (`688e364`)
- Pretrust managed-profile resume workspace (`6589315`)
- Remove quota identity duplicates (`3fbd892`)
- Migrate stored profile identity matching (`9a5585b`)
- Migrate login profile slug sanitizer (`8a5445d`)
- Dedupe provider profile catalogs (`5b1b825`)
- Classify profile import sources (`3549448`)
- Remove provider profile alias duplicate (`d53ef80`)

### Misc

- Align Codex 0.159.0 (`39a9d2c`)
- Migrate secret backend identity (`44d43df`)
- Migrate Copilot export host identity (`bfe7cdb`)
- Migrate Presidio identity policy (`3bf0f7d`)
- Migrate usage-limit text policy (`590098b`)
- Migrate session launch identity (`8837918`)
- Remove binary host casefold duplicates (`af224ba`)
- Add casefold substring search (`5e1b2ab`)
- Migrate provider scalar policy (`7d1313a`)
- Migrate Gemini tooling policy (`a32c1fe`)
- Migrate confirmation token policy (`faa7f85`)
- Migrate optional-tools policy (`821145c`)
- Migrate provider casefold policy (`b7901eb`)
- Add casefold text relations (`39efe4a`)
- Finish exact identity cleanup (`29cc6cf`)
- Remove exact identity duplicates (`2ec7f4d`)
- Migrate Gemini exact-output guardrails (`65d5d0c`)
- Migrate Gemini guardrail text policy (`62e44c1`)
- Centralize exact casefold equality (`9e0983b`)
- Resolve Kiro model identity (`8ef9a1c`)
- Resolve external catalog identity (`4a4385f`)
- Add exact catalog identity resolver (`5d8d37d`)
- Dedupe sub-agent catalogs (`b8bef27`)
- Dedupe bundled models in catalog (`4c1a5cd`)
- Centralize CI truth policy (`4047999`)
- Remove Gemini boolean duplicate (`b4640f6`)
- Migrate DeepSeek config token policy (`dea1626`)
- Hard-replace sub-agent launch policy (`dcd5741`)
- Centralize model provider classification (`1717673`)
- Migrate Kiro response tool-call policy (`b2d6a32`)
- Migrate Gemini stream metadata merge (`bb24169`)

## 0.433.0 - 2026-09-28

### Runtime

- Migrate runtime transcript semantics (`87bcb9a`)
- Require runtime tuning Mojo unconditionally (`263740d`)
- Migrate runtime lineage semantics (`3a10a02`)
- Migrate runtime state quota policy (`5923faa`)
- Migrate runtime proxy request policy (`0e5042b`)
- Migrate runtime state background policy (`49e8e08`)
- Remove runtime feature-off fallbacks (`036d655`)

### CLI

- Pretrust resume workspaces (`acb9773`)
- Migrate profile export policy limits (`6e49369`)
- Migrate quota selection policy (`a80b29f`)
- Migrate quota model policy (`f2795fd`)
- Migrate profile identity planning (`eea5e32`)
- Retire governance and quota rust oracles (`e6d850e`)

### Deps

- Bump the cargo group with 2 updates (`7170f87`)

### Misc

- Align with Codex 0.158.0 (`5c6421c`)
- Migrate broker guard and throughput policy (`afb7fec`)
- Migrate Super provider config policy (`cd9044d`)
- Migrate Super expose dispatch validation (`676b245`)
- Migrate sub-agent CLI policy (`71f1cbc`)
- Migrate Super expose protocol policy (`5c4638e`)
- Migrate provider usage policy (`169c27b`)
- Migrate DeepSeek metadata policy (`3761073`)
- Migrate route trace identifier policy (`4c05fae`)
- Merge pull request #97 from christiandoxa/dependabot/github_actions/github-actions-3a0b6790fd (`a0bf1ba`)
- Merge pull request #96 from christiandoxa/dependabot/cargo/cargo-b32a5ec21a (`e9f7189`)
- Migrate route decision reason policy (`d687a17`)
- Make Gemini stream shaping unconditional (`8353329`)
- Consolidate hard binding remember policy (`af08157`)
- Harden artifact line range parsing (`e48582b`)
- Migrate smart context semantic markers (`f3c84fc`)
- Migrate smart context artifact refs (`2374405`)
- Migrate session report metadata (`635ef58`)
- Migrate smart context repo map semantics (`53bd3da`)
- Consolidate Gemini tool assembly (`aec663e`)
- Migrate broker continuity semantics (`754f2a1`)
- Migrate state and codex config policy (`7b06bb4`)
- Migrate redaction semantics (`0cc1403`)
- Hard replace cli launch feature gates (`40822dd`)
- Remove prodex app feature fallbacks (`5271d68`)
- Hard replace provider response fallbacks (`d4bd318`)
- Remove Gemini bridge fallbacks (`f1df0c4`)
- Hard replace Gemini request translation (`416db36`)
- Hard replace DeepSeek stream shapes (`208df26`)
- Hard replace Kiro ACP shapes (`f5e0d64`)
- Hard replace Kiro response helpers (`9d612ba`)

## 0.432.2 - 2026-09-27

### Runtime

- Support Codex 0.157.1 resume (`4dec9c0`)
- Shorten overlays for Codex daemon sockets (`4ddf7d7`)
- Own continuation binding retention (`b496617`)
- Own continuation status policy (`04f03f7`)
- Own runtime log tokenization (`26ebebe`)

### CLI

- Aggregate OpenAI pools in Mojo (`8fc4c10`)
- Own quota status classification (`f70166a`)
- Own OpenAI model quota planning (`b78bd72`)

### Claude

- Move Anthropic response defaults into Mojo (`598b5f3`)
- Move Anthropic web-search shaping into Mojo (`42cf266`)

### Docs

- Record hard replacement authority and current share (`cf17b3c`)

### Misc

- Route Gemini text parts through Mojo (`1e38aac`)
- Move Kiro chat responses into Mojo (`430336a`)
- Move Gemini system instructions into Mojo (`c02469a`)
- Isolate MSVC weak aliases (`bf06276`)
- Support Codex 0.157.0 (`a96f995`)
- Merge pull request #95 from christiandoxa/mojo-75-chat-response-20260925 (`37f9f12`)
- Shape function history parts in Mojo (`376c06f`)
- Move label validation to Mojo (`8cd0b67`)
- Move request validation to Mojo (`87fba48`)
- Own provider error classification (`e5e4e0e`)
- Own Gemini stream event transform (`470f27d`)
- Own rate-limit header classification (`978d600`)
- Own SSE inspection transitions (`637d81b`)
- Own SSE line planning (`aafd19f`)
- Own response forwarding classification (`14b939d`)
- Record collapsed Smart Context adapters (`f458dda`)
- Own previous response error classification (`db4514b`)
- Own request compatibility surface planning (`ed73a21`)
- Own retry-after numeric policy (`12dc895`)
- Route remaining human status through ratatui (`cb04b3e`)
- Render interactive command status with ratatui (`bbaea3e`)
- Own operational log detail planning (`3506b8b`)
- Own operational log event classification (`b9b3343`)
