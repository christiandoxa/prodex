# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.288.0 - 2026-07-14

### Misc

- Add verified standalone installer (`04730d3`)

## 0.287.0 - 2026-07-14

- No grouped changes.

## 0.286.0 - 2026-07-14

### Runtime

- Retry quota-ready websocket profiles (`6e27fa4`)
- Apply resolved harness modes (`0902e4f`)

### CLI

- Expose harness selection (`fb434bf`)

### Misc

- Add harness policy core (`f4e73cd`)

## 0.285.0 - 2026-07-14

### Runtime

- Restore retries and large session resumes (`29eb66d`)

### CLI

- Keep partial usage profiles available (`8740c4b`)

### Misc

- Scan secrets inside quoted text (`4ebe905`)
- Add enterprise governance platform (`e07e2cc`)

## 0.284.0 - 2026-07-13

### CLI

- Trust hooks on launch (`b7c1f04`)

### Misc

- Add live monitoring dashboard (`82ca042`)

## 0.283.0 - 2026-07-13

### Misc

- Stabilize startup hooks and TUI borders (`e67a15d`)

## 0.282.0 - 2026-07-13

### Misc

- Add enterprise controls and Kiro parity (`fbc94c0`)

## 0.281.0 - 2026-07-13

- No grouped changes.

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
