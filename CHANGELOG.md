# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.375.0 - 2026-07-31

### Runtime

- Finish gateway module splits (`5fa71c0`)
- Preserve Gemini blocked response metadata (`0711553`)
- Satisfy Gemini Clippy checks (`f98d478`)
- Satisfy core clippy lints (`41d62be`)

### CLI

- Pass Windows descriptor pointer (`cded4b1`)

### Misc

- Latch poisoned audit writer (`7274d89`)
- Recover postcommit audits (`e1e6758`)
- Durable governance invalidation (`871a208`)
- Preserve committed mutation success (`e536986`)
- Close audited CLI and CI gaps (`cb3151f`)

## 0.374.0 - 2026-07-31

### Misc

- Make revision publication atomic (`b4f1ade`)

## 0.373.0 - 2026-07-30

### Misc

- Harden bank and control-plane boundaries (`8449b93`)

## 0.372.0 - 2026-07-30

### Misc

- Complete deployment and observability hardening (`f469cea`)

## 0.371.0 - 2026-07-30

### Misc

- Harden usage accounting (`f688973`)

## 0.370.0 - 2026-07-30

### Misc

- Harden cross-platform tooling and CI (`f95887c`)

## 0.369.0 - 2026-07-29

### Misc

- Clean partial Codex home copies (`9a099a1`)
- Skip Codex managed packages directory when copying CODEX_HOME (`953ad03`)

## 0.368.0 - 2026-07-29

### Misc

- Preserve postgres reconciliation parameter types (`1bf7ef6`)
- Log all guardrail webhook failures (`1537859`)
- Fail readiness on unavailable policy snapshots (`cd6cb74`)
- Make gateway accounting reconciliation durable (`649b9bf`)

## 0.367.0 - 2026-07-29

### Misc

- Close audited maintenance gaps (`fd0a751`)

## 0.366.0 - 2026-07-29

### Runtime

- Bound Windows proxy test shutdown (`8f377b8`)
- Harden Windows launch maintenance (`0cb8fe2`)
- Harden profile links and replay evidence (`50491ec`)

### Deps

- Sync Codex 0.146.0 lockfile (`0a64694`)

### Misc

- Support Codex 0.146.0 (`48c4b94`)

## 0.365.0 - 2026-07-28

### Misc

- Complete atomic mutation lifecycle (`5a957e6`)

## 0.364.0 - 2026-07-28

### Runtime

- Correct provider contracts and durable state writes (`c67df85`)
