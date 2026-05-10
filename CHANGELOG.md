# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.94.0 - 2026-05-10

### Runtime

- Split runtime maintenance surfaces (`f005d2b`)
- Extract websocket commit handling (`9200839`)
- Tighten runtime maintenance guardrails (`f2c5263`)
- Tighten runtime module boundaries (`51a2465`)
- Tighten runtime proxy maintenance boundaries (`ea2c75b`)
- Split runtime maintenance modules (`f7ff79f`)
- Reduce runtime support module size (`72d7b82`)
- Split runtime support modules (`21a9448`)
- Split runtime support modules (`da7b774`)
- Split large runtime modules (`161b72f`)

### Tests

- Split claude launch config coverage (`f579650`)
- Split affinity persistence requeue coverage (`2e58b31`)
- Split smart context and memory tests (`d234de6`)
- Stabilize claude native tool env guard (`61e1b56`)
- Stabilize route-specific backoff selection (`135349b`)

### CI

- Ratchet maintenance guardrails (`a507be2`)
- Clarify churn hygiene guidance (`060b605`)
- Update runtime manifest filters (`9f8cadd`)
- Recalibrate runtime proxy bench thresholds (`7d9512a`)
- Recognize root test extractions (`d50eb26`)
- Allow structural extraction churn (`a3fefbc`)
- Harden release hygiene safeguards (`b9826c6`)

### Misc

- Move token calibration persistence off hot path (`7e7d129`)
- Split smart context body rewriting (`12afad7`)
- Tighten maintenance guardrails (`5435680`)
- Ratchet maintenance boundaries (`8e6a9db`)
- Split smart context support (`de2a14e`)
- Silence selection helper reexports (`4b952d3`)

## 0.93.0 - 2026-05-09

### CI

- Rerun workflow after release guard (`5a4ed3b`)
- Streamline release and compat automation (`cfb2198`)
- Consolidate upstream compat tooling (`5e88ae9`)
- Consolidate release hygiene gate (`f902996`)
- Harden upstream compatibility watchdog (`693c77a`)
- Run release hygiene before push (`1dc85b9`)
- Refactor release hygiene guards (`f2973f5`)
- Enforce release hygiene guards (`f93c52c`)

### Misc

- Add safe housekeeping controls (`95ec69b`)
- Share Codex environments config (`9517257`)

## 0.92.0 - 2026-05-08

### Misc

- Harden caveman rtk launch overlays (`a02baf3`)

## 0.91.0 - 2026-05-08

### Misc

- Add rtk mode and repair session resume (`59d47a9`)

## 0.90.0 - 2026-05-08

- No grouped changes.

## 0.89.0 - 2026-05-08

### Misc

- Set Apache copyright notice (`51f67fa`)

## 0.88.0 - 2026-05-08

### CI

- Defer changelog freshness for non-release commits (`0e82478`)

### Misc

- Satisfy rust 1.95 clippy (`5c04ca3`)
- Sync package version 0.88.0 (`dee2036`)
- Trust caveman hooks for 0.129 (`e434bfd`)
- Support codex 0.129 transcript events (`91880c6`)
- Stabilize release section generation (`d5080b9`)

## 0.87.0 - 2026-05-07

### Runtime

- Extract smart context tool outputs (`87c9169`)
- Extract smart context types (`1bdc3bd`)
- Extract recall selection (`5ff71d1`)
- Extract smart context rehydration (`092b3a4`)
- Extract artifact refs (`6b38c2e`)
- Extract smart context repo state (`7748ac1`)
- Split schema and watch helpers (`8e0816e`)
- Extract smart context artifact refs (`cd31871`)
- Extract smart context modules (`552005b`)
- Split smart context internals (`7790eb4`)

### Tests

- Extract startup probe support cases (`9380958`)
- Extract claude launch support cases (`ea468e9`)
- Extract smart context golden corpus (`95ec42c`)
- Extract diagnostics output cases (`97d13a4`)
- Extract intent compaction cases (`67c82ba`)
- Extract git and search output cases (`da4a966`)
- Extract command output basics (`7f1dcd1`)
- Extract smart context budget cases (`33c7bdc`)
- Extract smart context semantic cases (`ba14a06`)
- Extract smart context alias cases (`9e701cc`)
- Extract smart context rehydration cases (`961e7bf`)
- Extract smart context manifest cases (`77bb0ca`)
- Extend smart context tool output split (`8751a11`)
- Split smart context tool output cases (`8585e5f`)

### CI

- Update claude launch shard filters (`a7df9b1`)
- Relax previous response hot path threshold (`26733f1`)
- Run churn hygiene in changed tests (`16402c8`)
- Enforce rust size guard (`520cedf`)
- Refresh upstream baseline (`d1a8be2`)
- Add rust size guard (`429b44b`)
- Add changed path test selector (`7b451c6`)
- Align prepush guard with push range (`7826a0e`)
- Add local prepush guard (`dce11d6`)

### Misc

- Split command output helpers (`1788c46`)
- Split command output compaction (`34b691d`)
- Split signal and blob noise modules (`cece18e`)

## 0.86.0 - 2026-05-07

### Runtime

- Recover auth and websocket precommit failures (`7fb8890`)

### CI

- Relax dead lineage bench threshold (`423ae81`)

## 0.85.0 - 2026-05-06

### Runtime

- Close smart context release gaps (`c978eb7`)

### CI

- Stabilize historical churn audits (`a3bd0b9`)

## 0.84.0 - 2026-05-06

### Runtime

- Preserve websocket after smart context panic (`0f7e77f`)

## 0.83.0 - 2026-05-06

### Runtime

- Close residual release gaps (`227d927`)
- Close residual release guard gaps (`51ba501`)
- Close runtime and release guard gaps (`8f9f61c`)

### CI

- Harden release guard calibration (`db3e2d6`)

### Misc

- Ignore changelog-only entries (`9b2746e`)
