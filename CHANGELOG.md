# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.383.0 - 2026-08-05

### Runtime

- Add sub-agent delegation and harden runtime (`c0c048a`)

## 0.382.0 - 2026-08-04

### Misc

- Close audited production gaps (`92d7bf8`)

## 0.381.0 - 2026-08-03

### Misc

- Support legacy glibc releases (`9126093`)
- Restore Windows open-file deletion (`1a163f1`)
- Preserve secure Windows sharing (`ceb30ca`)
- Restore cross-platform atomic writes (`25a5d0b`)
- Close audited reliability gaps (`710b4bc`)

## 0.380.0 - 2026-08-03

### Runtime

- Reject unsuccessful buffered terminal states (`bae6476`)
- Commit noncompact profile selection (`1feba09`)
- Preserve provider stream terminal failures (`e5d975d`)

### CLI

- Prevent implicit export races (`5f9cd3c`)
- Harden filesystem lifecycle (`679fcfd`)

### Misc

- Validate Redis ledger indexes atomically (`a085ce4`)
- Preserve ACP terminal states (`e0298b5`)

## 0.379.0 - 2026-08-03

### Deps

- Defer fuzz base64 0.23 (`92469a6`)
- Defer base64 0.23 (`4718816`)
- Bump rust from 1.97.0-bookworm to 1.97.1-bookworm (`c599d15`)
- Bump the fuzz-cargo group in /fuzz with 2 updates (`eea022c`)
- Bump the cargo group with 3 updates (`f82f090`)

### Misc

- Merge pull request #45 from christiandoxa/dependabot/docker/rust-1.97.1-bookworm (`b1eb26f`)
- Merge pull request #44 from christiandoxa/dependabot/github_actions/github-actions-5eb7864991 (`17f159b`)
- Merge pull request #43 from christiandoxa/dependabot/cargo/fuzz/fuzz-cargo-83702f5d0b (`6db4406`)
- Merge pull request #42 from christiandoxa/dependabot/cargo/cargo-8ee9223565 (`04bf95a`)

## 0.378.0 - 2026-08-02

### Runtime

- Close audited runtime and control-plane gaps (`e24f7aa`)

### Misc

- Close cross-platform CI regressions (`2d02ea8`)

## 0.377.0 - 2026-08-01

### Runtime

- Make Windows runtime tests deterministic (`5cb36da`)

### Misc

- Synchronize cross-platform log reads (`c52403a`)
- Make auto-redeem selection deterministic (`ebd9c44`)
- Eliminate Windows test stalls (`d716574`)
- Finish Windows test portability (`7d440ec`)
- Complete Windows CI portability (`c79b524`)
- Close audited cross-platform quality gaps (`a8320e5`)
- Close audited reliability gaps (`805983d`)

## 0.376.0 - 2026-07-31

- No grouped changes.

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
