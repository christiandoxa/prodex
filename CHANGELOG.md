# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.75.0 - Unreleased

Changes after `0.74.0`.

### CLI

- Preserve shared-account profile imports (`8703402`)

### Misc

- Improve smart context token efficiency (`8c82f51`)
- Reduce super context token overhead (`a20286e`)
- Improve super token efficiency (`b8012cd`)

## 0.74.0 - 2026-05-04

### CLI

- Prevent login profile email mismatch (`618664f`)

### Misc

- Release 0.74.0 (`a15618d`)

## 0.73.0 - 2026-05-04

### Misc

- Add smart context autopilot (`7b2a9d7`)

## 0.72.0 - 2026-05-04

### Tests

- Colocate crate tests under crate directories (`1c65070`)

### Misc

- Release 0.72.0 (`dde529f`)
- Satisfy smart context clippy gate (`b432d95`)
- Add smart context autopilot (`7d98725`)

## 0.71.0 - 2026-05-03

### Misc

- Release 0.71.0 (`84a71b4`)
- Update refactor guard paths (`30d95c9`)
- Move app code and tests out of src (`4014e29`)

## 0.70.0 - 2026-05-03

### Runtime

- Move session and runtime state helpers into crates (`dcab284`)
- Move runtime selection support into crates (`d22f2a7`)
- Extract runtime continuity metrics store (`405f984`)
- Extract runtime launch profile helpers (`21c3bff`)
- Extract runtime sse tap state (`3a73641`)
- Extract runtime quota adapter crate (`e923a7b`)
- Move runtime state helpers into store crate (`fd7120c`)
- Extract runtime diagnostics helpers into crates (`4203453`)
- Extract runtime helpers into crates (`b1a9e58`)
- Split runtime logic into workspace crates (`a453d31`)
- Extract app and runtime helpers into crates (`24f85c2`)
- Format runtime doctor imports (`a56d3ae`)
- Extract runtime cookie relay crate (`c4e2428`)
- Extract runtime store helpers (`c12030d`)
- Extract runtime broker helpers (`1a721f6`)
- Extract runtime tuning helpers (`63d5172`)
- Move proxy helpers into runtime crates (`686aa1f`)
- Extract claude and caveman runtime helpers (`d9c3404`)
- Extract launch and proxy config crates (`6c2ca07`)

### CLI

- Require email match for profile identity reuse (`b62a240`)
- Move profile export io into crate (`193020b`)
- Satisfy clippy profile export layout (`6809213`)

### Tests

- Move websocket executor coverage into crate (`089eba8`)
- Move child launch coverage into crate (`98f1e94`)
- Stabilize compact overload marker assertion (`bbc3925`)

### CI

- Relax lineage cleanup bench threshold (`68148ff`)
- Relax dead lineage bench threshold (`55518d9`)
- Relax sse bench threshold (`9f6d8b6`)
- Refresh upstream compatibility baseline (`9b8e1ea`)

### Misc

- Release 0.70.0 (`17a42cf`)
- Extract housekeeping helpers (`2c96bc4`)
- Satisfy clippy previous response orchestration (`e5b292c`)
- Move more src helpers into crates (`746f0da`)
- Extract src helpers into workspace crates (`7474281`)
- Extract context helpers into crate (`7404c0a`)
- Split modules into workspace crates (`18459e9`)

## 0.69.0 - 2026-05-01

### CLI

- Satisfy quota clippy gate (`fb4265e`)

### CI

- Serialize auto-rotate integration tests (`6a00aee`)

### Misc

- Split internal workspace crates (`7881ca4`)

## 0.68.0 - 2026-04-30

### Runtime

- Promote fresh websocket rotations (`00dcc41`)
- Make auto-rotate opt-in (`616c8d0`)
- Split runtime support crates (`11bddfb`)

### CLI

- Add session inspection and quota auth filtering (`24472a0`)

### CI

- Relax sse bench smoke threshold (`203afbd`)
- Update split crate lint checks (`49536ce`)

### Misc

- Satisfy clippy for terminal ui crate (`5d36706`)
- Split additional support crates (`0c38b27`)
- Split leaf modules into workspace crates (`2f20685`)

## 0.67.0 - 2026-04-29

### Runtime

- Improve token efficiency and websocket proxying (`bc1d7b1`)

### Docs

- Add support section to README with donation link (`ba124dd`)

### CI

- Refresh upstream watchdog baseline (`a5ff040`)
- Satisfy clippy warning gate (`bad9f5e`)

## 0.66.0 - 2026-04-29

### Runtime

- Support proxied Codex launches (`990eb56`)

## 0.65.0 - 2026-04-29

### CLI

- Support OpenAI workspace identities (`764bf2b`)

## 0.64.0 - 2026-04-29

### Runtime

- Add prompt cache affinity (`8110553`)
- Harden runtime proxy validation (`a088875`)

## 0.63.0 - 2026-04-28

### Runtime

- Support proxies and native Codex history (`63ef05a`)
