# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.408.9 - 2026-08-18

### Docs

- Record evidence-based audit discipline (`e129baa`)
- Keep README installer on latest (`895a610`)
- Install latest release by default (`1e00977`)

### Misc

- Revert "chore(release): release 0.408.9" (`237b17b`)
- Reconcile sessions and persist model preferences (`d84ad90`)

## 0.408.8 - 2026-08-16

### Runtime

- Fix audited runtime, storage, release docs, and CI (`847efd6`)

## 0.408.7 - 2026-08-16

### CLI

- Publish refresh result after heartbeat timeout (`236d88d`)

### Misc

- Close audited gateway and provider gaps (`cdfd4c2`)

## 0.408.6 - 2026-08-15

### Runtime

- Close audited runtime continuity and CI gaps (`8df9654`)

### Docs

- Pin latest release install commands (`b07504c`)

## 0.408.5 - 2026-08-15

### Runtime

- Close audited runtime security UX and CI gaps (`5d3e509`)

## 0.408.4 - 2026-08-15

### Misc

- Harden gateway headers and release tooling (`4709593`)

## 0.408.3 - 2026-08-14

### Runtime

- Harden runtime rotation and provider selection (`5516be2`)

### CLI

- Align Kiro effort and persisted quota fallback (`88286e5`)
- Preserve model and quota readiness across profiles (`554bc5a`)

## 0.408.2 - 2026-08-13

### CLI

- Skip catalog startup (`1bb3220`)
- Include weekly-ready OpenAI profiles (`a3c5b5d`)
- Use ready quota snapshots (`0375a53`)
- Continue after profile launch errors (`169ac6b`)

### Misc

- Preserve UUID identifiers (`4815838`)
- Clean live broker descendants (`eaa5bcf`)
- Scope provider-core to tests (`c0e0c17`)
- Close diagnostic secret boundary leaks (`821ad97`)
- Serialize backup recovery repairs (`ba002e0`)

## 0.408.1 - 2026-08-12

### Runtime

- Harden log completion and auth caching (`85b98d4`)

## 0.408.0 - 2026-08-12

### Runtime

- Preserve first-pass profile rotation (`53dd38b`)
- Persist quota readiness across profiles (`20c4bf7`)

### CLI

- Restore weekly-only availability (`ee3d4f1`)

## 0.407.4 - 2026-08-12

### Runtime

- Recover healthy profile selection (`4c68738`)
- Improve proxy recovery and payload logging (`57c2e3c`)
- Preserve websocket owner recovery (`42d4643`)
- Rebind rotated sessions (`a531a87`)
- Recover stale session bindings (`e0221f6`)
- Honor fresh websocket owner retry (`553f27a`)
- Preserve affinity under selection pressure (`f94e3db`)
- Rotate exhausted resumed sessions (`1b597df`)
- Restore quota rotation and speed releases (`4d17333`)
- Harden runtime paths and CI parallelism (`fc97b65`)
- Harden websocket and sub-agent execution (`3f699c0`)
- Simplify websocket session admission (`24b51e2`)
- Preserve local saturation and child cleanup (`e956baf`)
- Satisfy clippy argument lint (`38abfdc`)
- Bound fresh profile admission and preserve Kiro models (`299a462`)
- Keep Kiro process modules within size guard (`77d5919`)
- Complete Kiro sub-agent turns and harden installer (`f7d63a7`)
- Preserve sub-agent provider compatibility (`5f23df7`)
- Stabilize macOS broker recovery (`5de7743`)
- Add sub-agent delegation and harden runtime (`c0c048a`)
- Reject unsuccessful buffered terminal states (`bae6476`)
- Commit noncompact profile selection (`1feba09`)
- Preserve provider stream terminal failures (`e5d975d`)
- Close audited runtime and control-plane gaps (`e24f7aa`)
- Make Windows runtime tests deterministic (`5cb36da`)
- Finish gateway module splits (`5fa71c0`)
- Preserve Gemini blocked response metadata (`0711553`)
- Satisfy Gemini Clippy checks (`f98d478`)
- Satisfy core clippy lints (`41d62be`)
- Bound Windows proxy test shutdown (`8f377b8`)
- Harden Windows launch maintenance (`0cb8fe2`)
- Harden profile links and replay evidence (`50491ec`)
- Correct provider contracts and durable state writes (`c67df85`)
- Harden runtime compatibility edge cases (`ec50608`)
- Close audited implementation gaps (`6200d1d`)
- Close provider and runtime audit gaps (`65f5028`)
- Harden Gemini compatibility (`67d338b`)
- Close runtime governance gaps (`0c5e2ce`)
- Harden log, MCP, and Gemini boundaries (`fe12705`)
- Complete audited security paths (`aaaf603`)
- Align secure log test fixtures (`32d599f`)
- Sync runtime test manifest (`dde40db`)
- Harden runtime and release pipeline (`e9ed1e0`)
- Close audited runtime and governance gaps (`014ce83`)
- Harden distributed runtime boundaries (`71f9ea5`)
- Harden timeout and secret cleanup (`078701b`)
- Isolate provider conversation state (`b0dc453`)
- Support Codex realtime v3 passthrough (`96a4908`)
- Complete audited runtime and operations features (`9c96750`)
- Harden Codex input on VTE terminals (`5095669`)
- Honor Presidio opt-in and fail-open (`86ae8bf`)
- Complete audited runtime and governance features (`f5c4302`)
- Preserve Presidio JSON field boundaries (`6364e68`)
- Skip tool schemas during PII inspection (`68fc84e`)
- Allow large Codex request bodies (`7630aa4`)
- Launch Copilot through Prodex proxy (`923258b`)
- Harden accounting and broker reliability (`cdb3035`)
- Harden audited runtime and gateway features (`6e906af`)
- Resume quota-rejected goals across profiles (`38599f0`)
- Retry quota-ready transport fallback (`9bf8b58`)
- Rotate past weekly-exhausted profiles (`f39b472`)
- Resume live goals across profiles (`77a3015`)
- Translate compact requests for local providers (`e8e136b`)
- Retry quota-ready websocket profiles (`aaa5aaf`)
- Apply resolved harness modes (`aef66e4`)
- Restore retries and large session resumes (`68abbd9`)
- Preserve weekly-only websocket sessions (`0090261`)
- Normalize partial runtime windows (`dbebd3a`)
- Preserve smart context fault fallback (`6a9a3ef`)
- Redact runtime request secrets (`2e2899e`)
- Capture proxy wait durations (`2ff31ea`)
- Enforce typed service modes (`5b6867a`)
- Harden runtime and application boundaries (`f62dc68`)

### CLI

- Harden child tool launches (`8e2f274`)
- Preserve quota snapshots and aliases (`b5b6325`)
- Support trusted imports and Codex 0.147.0 (`1ac7b11`)
- Trim effort fallback (`15b3a1c`)
- Expose sub-agent effort choices (`83e97dd`)
- Harden child and Kiro process cleanup (`9897913`)
- Keep launcher guidance within size guard (`b764e44`)
- Clarify child launcher contract (`33cd4f3`)
- Tolerate slow Windows heartbeats (`f3cfa3c`)
- Harden provider delegation (`e865fac`)
- Prevent implicit export races (`5f9cd3c`)
- Harden filesystem lifecycle (`679fcfd`)
- Pass Windows descriptor pointer (`cded4b1`)
- Restore Presidio opt-in prompt (`7fd29c3`)
- Use local Ponytail marketplace source (`91c8e75`)
- Restore prompt-free yolo launches (`b2de2cd`)
- Harden cross-platform quota preflight (`ebc4e96`)
- Keep paste guard out of exec (`f1d27d2`)
- Show process list and harden TUI input (`1c708d6`)
- Harden quota retry, terminal input, and update freshness (`4ca3c46`)
- Preserve Windows ACL handle rights (`35f520f`)
- Harden Windows lease contention (`45fb27c`)
- Preserve partial quota windows (`477823e`)
- Restore cross-profile desktop history (`73f7695`)
- Persist chat history index (`07cdcec`)
- Enable native keyring storage (`1c3fd26`)
- Add playwright shortcut (`858d3e0`)
- Keep refresh lease records readable on Windows (`7a7d19e`)
- Recheck exhausted launch snapshots (`61b6ed7`)
- Expose harness selection (`7ed0044`)
- Keep partial usage profiles available (`5b41e29`)
- Trust hooks on launch (`0cbf577`)
- Allow weekly-only quota snapshots (`c8c94a4`)
- Render partial quota windows accurately (`d93d554`)
- Support partial Spark windows (`b73215d`)
- Allow missing rate limit windows (`c1b6073`)
- Redact quota URL arguments (`afa5412`)
- Zeroize quota auth credentials (`5fb455e`)
- Redact profile auth identity tokens (`66cda50`)
- Harden profile export files (`c7f4218`)
- Defer gateway outbound bearer resolution (`95a7fe6`)
- Remove raw profile export keys (`05874b4`)

### Claude

- Stabilize optional MCP cold startup (`e3d0f87`)
- Harden optional MCP startup probes (`4815d05`)

### Docs

- Strengthen agent guidance (`171c206`)
- Refresh smart context evidence (`d0c04f4`)
- Include fuzz lock in manual releases (`93f5c30`)
- Document lockfile refresh on workspace releases (`55beda1`)
- Restore README banner and format (`ffcf905`)
- Refresh smart context evidence (`5657092`)
- Record final hardening validation (`5b4b0e6`)
- Consolidate hardening guidance (`da1b2ab`)
- Clarify Copilot native response transport (`2b289c4`)
- Allow configured commit identity (`433b0f4`)
- Record atomic admin cutover (`d299062`)
- Record post-cutover samples (`509d19b`)
- Record provider secret evidence (`205f353`)
- Correct security completion matrix (`1a8eff5`)
- Record validation and benchmark evidence (`8af7fbf`)
- Document hardened production boundaries (`36a99a9`)

### Deps

- Bump debian from `7b140f3` to `abd67ff` (`b16425a`)
- Bump rust from `77fac8b` to `14bc9c5` (`7fc6998`)
- Bump the fuzz-cargo group in /fuzz with 2 updates (`00ee0c1`)
- Bump the cargo group with 2 updates (`d4d1bd8`)
- Defer fuzz base64 0.23 (`92469a6`)
- Defer base64 0.23 (`4718816`)
- Bump rust from 1.97.0-bookworm to 1.97.1-bookworm (`c599d15`)
- Bump the fuzz-cargo group in /fuzz with 2 updates (`eea022c`)
- Bump the cargo group with 3 updates (`f82f090`)
- Sync Codex 0.146.0 lockfile (`0a64694`)
- Sync fuzz lockfile (`193f043`)
- Accommodate base64 version split (`d4e29b8`)
- Update Rust image to 1.97.1 (#39) (`f5c9da4`)
- Merge Dependabot fuzz updates (#37) (`3127c06`)
- Merge Dependabot cargo updates (#36) (`37fd40d`)
- Bump rust from 1.97.0-bookworm to 1.97.1-bookworm (`fef1e62`)
- Bump the fuzz-cargo group in /fuzz with 2 updates (`c1e8742`)
- Bump the cargo group with 2 updates (`853a9f1`)
- Bump serde from 1.0.228 to 1.0.229 in the cargo group (#31) (`1c9251a`)
- Bump debian from `60eac75` to `7b140f3` (#35) (`7c093ba`)
- Bump rust from `7d0723d` to `8fa55b2` (#34) (`1fe6689`)
- Bump the fuzz-cargo group in /fuzz with 2 updates (#32) (`db8aa3a`)
- Remove unused postgres dependency (`17a68be`)
- Classify test-only dependencies (`cf4d7fc`)
- Merge pull request #27 (`eeaaeb8`)
- Bump uuid in the cargo group across 1 directory (`520c6bf`)
- Merge pull request #26 (`d729e31`)
- Merge pull request #25 (`f6650ca`)
- Bump serde_json (`7f941a0`)
- Bump tungstenite from 0.29.0 to 0.30.0 in the cargo group (`4e4d735`)

### Misc

- Show provider models (`344f3d6`)
- Compile provider process cleanup (`ad9a559`)
- Bound SDK requests and responses (`27bc17b`)
- Bound external command lifecycles (`6191a6c`)
- Share codebase memory across agents (`c658b7d`)
- Stabilize Kiro and Copilot sessions (`0571169`)
- Keep interactive TUI in terminal foreground group (`6718f56`)
- Enable all Super optional tools (`1298966`)
- Ratchet presidio size guard (`76bcc08`)
- Satisfy sonar complexity gates (`3c877b2`)
- Satisfy clippy quality gates (`db4d929`)
- Use local Ponytail marketplace source (`87cf504`)
- Satisfy clippy test ordering (`0e6c609`)
- Skip ponytail when node is unavailable (`625c4ae`)
- Set interactive window title (`93888f4`)
- Merge pull request #47 from kbakdev/fix/install_on_windows (`41cf6c0`)
- Improve architecture detection for Windows (`1c7bb25`)
- Reuse native CLI credentials (`ccf5f27`)
- Strip parent external tools (`1e46eb6`)
- Sync Codex installer and fuzz metadata (`a8682ee`)
- Track Codex 0.146.1 baseline (`b1eac18`)
- Close audited production gaps (`92d7bf8`)
- Support legacy glibc releases (`9126093`)
- Restore Windows open-file deletion (`1a163f1`)
- Preserve secure Windows sharing (`ceb30ca`)
- Restore cross-platform atomic writes (`25a5d0b`)
- Close audited reliability gaps (`710b4bc`)
- Validate Redis ledger indexes atomically (`a085ce4`)
- Preserve ACP terminal states (`e0298b5`)
- Merge pull request #45 from christiandoxa/dependabot/docker/rust-1.97.1-bookworm (`b1eb26f`)
- Merge pull request #44 from christiandoxa/dependabot/github_actions/github-actions-5eb7864991 (`17f159b`)
- Merge pull request #43 from christiandoxa/dependabot/cargo/fuzz/fuzz-cargo-83702f5d0b (`6db4406`)
- Merge pull request #42 from christiandoxa/dependabot/cargo/cargo-8ee9223565 (`04bf95a`)
- Close cross-platform CI regressions (`2d02ea8`)
- Synchronize cross-platform log reads (`c52403a`)
- Make auto-redeem selection deterministic (`ebd9c44`)
- Eliminate Windows test stalls (`d716574`)
- Finish Windows test portability (`7d440ec`)
- Complete Windows CI portability (`c79b524`)
- Close audited cross-platform quality gaps (`a8320e5`)
- Close audited reliability gaps (`805983d`)
- Latch poisoned audit writer (`7274d89`)
- Recover postcommit audits (`e1e6758`)
- Durable governance invalidation (`871a208`)
- Preserve committed mutation success (`e536986`)
- Close audited CLI and CI gaps (`cb3151f`)
- Make revision publication atomic (`b4f1ade`)
- Harden bank and control-plane boundaries (`8449b93`)
- Complete deployment and observability hardening (`f469cea`)
- Harden usage accounting (`f688973`)
- Harden cross-platform tooling and CI (`f95887c`)
- Clean partial Codex home copies (`9a099a1`)
- Skip Codex managed packages directory when copying CODEX_HOME (`953ad03`)
- Preserve postgres reconciliation parameter types (`1bf7ef6`)
- Log all guardrail webhook failures (`1537859`)
- Fail readiness on unavailable policy snapshots (`cd6cb74`)
- Make gateway accounting reconciliation durable (`649b9bf`)
- Close audited maintenance gaps (`fd0a751`)
- Support Codex 0.146.0 (`48c4b94`)
- Complete atomic mutation lifecycle (`5a957e6`)
- Close audited source gaps (`853f0da`)
- Close audited release gaps (`0735f52`)
- Isolate Gemini host files (`fb963d4`)
- Harden artifact trust and telemetry (`5332471`)
- Harden file checks and test coverage (`2af9e9d`)
- Split full app tests from platform matrix (`8bc5170`)
- Route stderr size queries through backend (`2627508`)
- Initialize stderr TUI without /dev/tty (`2770426`)
- Close audited release gaps (`6b02064`)
- Complete audited production paths (`56e2dfd`)
- Reuse exact round-trip validation (`36c818b`)
- Close hardening validation regressions (`5cce0a3`)
- Pin Windows Codex migration (`4eac31d`)
- Publish Smart Context performance evidence (`04063d3`)
- Bound rewrite work (`de60d09`)
- Reject no-op before tokenization (`08f3fb1`)
- Pin Codex npm artifacts (`fcd6d78`)
- Share artifact snapshots (`160a269`)
- Validate checked subprocess output (`55a816c`)
- Make super least privilege by default (`6e2d44f`)
- Execute smart context replay corpus (`8fd3d53`)
- Make smart context rewrites transactional (`a6b376c`)
- Secure optimizer cache state (`9efa10b`)
- Complete gateway lifecycle and provider health (`a001973`)
- Close provider and governance gaps (`61116c9`)
- Support Windows session attachment paths (`9330667`)
- Harden cross-platform session repair (`1297235`)
- Normalize Windows context paths (`0421672`)
- Make embedded asset checks cross-platform (`a811276`)
- Accept trusted Windows child owners (`e0fe68c`)
- Seal cross-platform login secrets (`d7905a9`)
- Complete native Windows support (`d9f1e44`)
- Make PTY tests portable to macOS (`d435275`)
- Close remaining audited gaps (`741553c`)
- Harden response enforcement (`c675728`)
- Harden provider launches and session recovery (`5d6a757`)
- Harden process reporting and internal helpers (`7cb9042`)
- Close audited production gaps (`d6b0a3a`)
- Close audited governance gaps (`253e493`)
- Bound metadata discovery processes (`789cd2c`)
- Decode readable zstd payloads (`626d2cd`)
- Complete audited Codex and governance passthrough (`6f955c3`)
- Complete audited feature implementations (`650bfa9`)
- Document adaptive route decision (`edea3bb`)
- Detect current legacy schema (`42072b6`)
- Persist desktop chat history (`afd98e5`)
- Remove unimplemented audited surfaces (`f64288e`)
- Harden audited desktop and dashboard paths (`3c020c2`)
- Launch platform Codex desktop apps (`2ac4220`)
- Add OpenCodex-inspired control center (`3bb5996`)
- Preserve explicit route pricing (`2d08451`)
- Harden clients and enterprise gateway (`f4b40a3`)
- Reject inactive adaptive routing (`9a43ebe`)
- Harden native client launches (`31337b2`)
- Add immutable implementation registry (`0f18395`)
- Avoid unauthenticated GitHub API requests (`35b7c37`)
- Implement evaluated provider policies (`b5cea54`)
- Harden enterprise gateway controls (`ca16c48`)
- Harden Windows checksum verification (`799c0ae`)
- Add verified standalone installer (`7fcb193`)
- Add harness policy core (`128f0fd`)
- Scan secrets inside quoted text (`9670fa3`)
- Add enterprise governance platform (`b893b21`)
- Add live monitoring dashboard (`886ed70`)
- Stabilize startup hooks and TUI borders (`afbfaed`)
- Add enterprise controls and Kiro parity (`6dd5be2`)
- Migrate legacy prodex root permissions (`84c676a`)
- Normalize apple sticky bit mask (`a301b8e`)
- Own private windows files (`3a2785b`)
- Request write access for windows temp files (`d9a030d`)
- Gate allocator evidence example (`8827f32`)
- Ensure atomic action tenant exists (`05d6d7a`)
- Support scim replace routing (`5a5cec9`)
- Bind admin actions to exact resources (`7b2cf67`)
- Add atomic admin mutation storage (`0fbec79`)
- Rotate key secrets with patches atomically (`7312bbc`)
- Preserve patch-based key rotation (`f70681d`)
- Canonicalize audit and idempotency digests (`42beb40`)
- Harden enterprise credential boundaries (`22e6e31`)
- Filter provider transform headers (`ab69442`)
- Zeroize Copilot config tokens (`79bb95b`)
- Reject credential-bearing service URLs (`2697997`)
- Reject credentialed gateway endpoints (`2799dee`)
- Validate Presidio service URLs (`cc36daa`)
- Authorize admin mutations in transaction (`570973b`)
- Add dedicated gateway mode (`cfcfbc5`)
- Isolate resumed provider env (`46082b0`)
- Isolate provider secrets from child env (`ef00639`)
- Cut over dedicated in-process serving (`a773bbe`)
- Add in-process application transport (`5dff9a4`)
- Expose allocation benchmark evidence (`4687178`)
- Activate control-plane service (`8fec98c`)
- Add opt-in allocation counters (`9cb2c66`)
- Add bounded in-process transport (`2ffa810`)
- Add canonical request context (`daf4028`)
- Move Gemini Live key to header (`9746517`)
- Validate verified credential evidence (`39c6a3c`)
- Defer projected provider secrets (`3b28040`)
- Project compose gateway secrets (`938bd52`)
- Add in-process request handler (`c19b7e9`)
- Harden broker capability files (`4df6196`)
- Require control-plane idempotency (`85b0250`)
- Project migration database secrets (`624a64e`)
- Add explainable constraint-aware routing (`a490169`)
