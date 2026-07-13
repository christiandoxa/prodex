# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.280.0 - 2026-07-13

### Runtime

- Preserve weekly-only websocket sessions (`319b415`)
- Normalize partial runtime windows (`b96e335`)

### CLI

- Allow weekly-only quota snapshots (`5568e50`)
- Render partial quota windows accurately (`a201051`)
- Support partial Spark windows (`a63c138`)

### Deps

- Merge pull request #27 (`b78df11`)
- Bump uuid in the cargo group across 1 directory (`7362012`)

## 0.279.0 - 2026-07-13

### CLI

- Allow missing rate limit windows (`686b462`)

### Docs

- Allow configured commit identity (`9da7f01`)

## 0.278.0 - 2026-07-12

### Deps

- Merge pull request #26 (`5b5c912`)
- Merge pull request #25 (`107ee85`)
- Bump serde_json (`3d7c05f`)
- Bump tungstenite from 0.29.0 to 0.30.0 in the cargo group (`0ab418d`)

### Misc

- Migrate legacy prodex root permissions (`5b03d95`)

## 0.277.0 - 2026-07-12

### Runtime

- Preserve smart context fault fallback (`a2dcfb3`)
- Redact runtime request secrets (`a6666f1`)
- Capture proxy wait durations (`609dccd`)
- Enforce typed service modes (`406dd6a`)
- Harden runtime and application boundaries (`c345662`)

### CLI

- Redact quota URL arguments (`2bd6e51`)
- Zeroize quota auth credentials (`b1287cb`)
- Redact profile auth identity tokens (`b7d23ae`)
- Harden profile export files (`a08507b`)
- Defer gateway outbound bearer resolution (`08a43e3`)
- Remove raw profile export keys (`0afc344`)

### Docs

- Record atomic admin cutover (`6f0c97e`)
- Record post-cutover samples (`65f0e2a`)
- Record provider secret evidence (`1d620cc`)
- Correct security completion matrix (`6b41bb9`)
- Record validation and benchmark evidence (`6863a43`)
- Document hardened production boundaries (`b4aff53`)

### Misc

- Normalize apple sticky bit mask (`10d41c9`)
- Own private windows files (`42dc008`)
- Request write access for windows temp files (`4a66527`)
- Gate allocator evidence example (`7a20b62`)
- Ensure atomic action tenant exists (`78904db`)
- Support scim replace routing (`b02c123`)
- Bind admin actions to exact resources (`a190eb9`)
- Add atomic admin mutation storage (`6d58224`)
- Rotate key secrets with patches atomically (`93a9951`)
- Preserve patch-based key rotation (`9921561`)
- Canonicalize audit and idempotency digests (`40059a4`)
- Harden enterprise credential boundaries (`f34310b`)
- Filter provider transform headers (`ed6c425`)
- Zeroize Copilot config tokens (`762c3b1`)
- Reject credential-bearing service URLs (`dc2c8a9`)
- Reject credentialed gateway endpoints (`fd2c6da`)
- Validate Presidio service URLs (`08f7ea5`)
- Authorize admin mutations in transaction (`2dd354b`)
- Add dedicated gateway mode (`17e995c`)
- Isolate resumed provider env (`0eb8f4d`)
- Isolate provider secrets from child env (`b1edbab`)
- Cut over dedicated in-process serving (`f6589f8`)
- Add in-process application transport (`c934e0c`)
- Expose allocation benchmark evidence (`b209977`)
- Activate control-plane service (`dec4b42`)
- Add opt-in allocation counters (`bab6ab1`)
- Add bounded in-process transport (`8b775a1`)
- Add canonical request context (`8071e04`)
- Move Gemini Live key to header (`d56c91b`)
- Validate verified credential evidence (`06cff11`)
- Defer projected provider secrets (`5dad6f5`)
- Project compose gateway secrets (`b492099`)
- Add in-process request handler (`df5615a`)
- Harden broker capability files (`d2830fd`)
- Require control-plane idempotency (`1d4592d`)
- Project migration database secrets (`546bbae`)

## 0.276.0 - 2026-07-11

### Misc

- Add explainable constraint-aware routing (`b315a0e`)

## 0.275.0 - 2026-07-11

### Runtime

- Preserve quota retry and repair transcripts (`afdb1bd`)

## 0.274.0 - 2026-07-11

### Runtime

- Add async redis limiter runtime (`57a7e3e`)
- Add pooled postgres runtime (`df800b4`)
- Harden enterprise gateway runtime (`08e2200`)
- Export config publication runtime delivery adapter (`3c3211b`)
- Clean npm smoke and runtime policy test-only helpers (`c98918f`)

### CLI

- Rotate gateway credentials atomically (`1a070dd`)
- Pin projected secret generation (`6074230`)
- Add development provider (`926ba22`)
- Wire production gateway references (`d30bd49`)
- Add projected external provider (`492fd82`)

### Docs

- Align migration job runbook (`89f25a1`)

### Misc

- Replace unmaintained pem parser (`a2218eb`)
- Enforce postgres transport tls (`147e6e5`)
- Enforce grouped request budgets (`b97ed3d`)
- Enforce distributed rpm tpm (`2c3a02e`)
- Add atomic redis rpm tpm admission (`9ef8c6c`)
- Wire redis coordination reference (`4fd9d5d`)
- Wire pooled postgres accounting (`34f3d54`)
- Wire dedicated async serve (`95bbd73`)
- Add async compatibility front (`9184705`)
- Drain on shutdown signals (`20d736a`)
- Project gateway secrets (`2981d57`)
- Make postgres rls migration repeatable (`29a120f`)
- Preserve last-known-good on reload failure (`39e69af`)
- Mature provider gateway and control plane (`cef3d8d`)
- Validate legacy admin ledger pagination queries (`fc5ba2b`)
- Delegate admin governance headers to shared boundary (`c2d5028`)
- Add control-plane http planning command (`b8a5147`)
- Add control-plane publication planning command (`d09a5c0`)
- Add control-plane publication delivery command (`c8253c3`)
- Unblock local preflight and provider bridge tests (`aa74651`)
- Checkpoint enterprise readiness refactor (`71e4274`)

## 0.273.0 - 2026-07-10

### CLI

- Skip workspace trust prompt (`0c2e464`)

## 0.272.0 - 2026-07-10

### Runtime

- Harden gateway and proxy boundaries (`1fc05d4`)

### CLI

- Simplify optimizer candidate lookup (`11ebc21`)

## 0.270.0 - 2026-07-10

### CLI

- Trust launch directory in yolo mode (`436bce1`)

## 0.269.0 - 2026-07-10

### Misc

- Protect active session transcripts (`f2113d1`)

## 0.268.0 - 2026-07-10

### Runtime

- Harden runtime compatibility and launch handling (`1476ca5`)
