# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.82.0 - Unreleased

Changes after `0.81.0`.

### Runtime

- Harden smart context rewrite fallback (`80f4a87`)

### Docs

- Record release sync ci fix (`4b8ac11`)

### CI

- Fetch full history for release sync (`3a2c6d8`)

## 0.81.0 - 2026-05-06

### Runtime

- Stabilize websocket streaming (`a92f29a`)

## 0.80.0 - 2026-05-06

### Runtime

- Avoid broker readiness timeout (`8bc2a85`)
- Reduce compact pressure for affinity continuations (`dee2401`)
- Reduce default smart context token overhead (`4a64cc5`)

## 0.79.0 - 2026-05-05

### CLI

- Improve embedded token efficiency (`37bf326`)

### Misc

- Improve super token efficiency (`7072bc3`)
- Satisfy ci clippy gate (`a895b53`)
- Optimize smart context token usage (`0cc7747`)
- Improve smart context token efficiency (`93b4f97`)

## 0.78.0 - 2026-05-05

### CLI

- Improve embedded token efficiency (`8fbb819`)
- Trim embedded token overhead (`449d145`)
- Reduce smart context token overhead (`a65ef71`)
- Improve token-efficient context calibration (`fe27e49`)
- Tighten smart context token budgets (`c13709c`)
- Reduce smart context token overhead (`0e68795`)
- Reduce embedded context token overhead (`a6cd544`)

### Tests

- Relax smart context rewrite timeout (`b0df72d`)

### CI

- Satisfy clippy warning gate (`5bcad24`)

### Misc

- Sanitize personal fixture data (`0c3e8b5`)

## 0.77.0 - 2026-05-05

### CLI

- Trim smart context token overhead (`9ad2f58`)
- Improve token-efficient context compaction (`d17c97f`)
- Reduce context token overhead (`f4b8c71`)

### CI

- Satisfy clippy warning gate (`05544d4`)

## 0.76.0 - 2026-05-04

### CLI

- Improve prodex super token efficiency (`e7eb552`)

## 0.75.0 - 2026-05-04

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
