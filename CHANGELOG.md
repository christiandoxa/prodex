# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.389.0 - 2026-08-06

### CLI

- Trim effort fallback (`15b3a1c`)
- Expose sub-agent effort choices (`83e97dd`)

## 0.388.0 - 2026-08-06

### Misc

- Reuse native CLI credentials (`ccf5f27`)

## 0.387.0 - 2026-08-06

### Misc

- Strip parent external tools (`1e46eb6`)

## 0.386.0 - 2026-08-06

### CLI

- Harden child and Kiro process cleanup (`9897913`)
- Keep launcher guidance within size guard (`b764e44`)
- Clarify child launcher contract (`33cd4f3`)

## 0.385.0 - 2026-08-06

### Runtime

- Preserve sub-agent provider compatibility (`5f23df7`)

### CLI

- Tolerate slow Windows heartbeats (`f3cfa3c`)

### Misc

- Sync Codex installer and fuzz metadata (`a8682ee`)
- Track Codex 0.146.1 baseline (`b1eac18`)

## 0.384.0 - 2026-08-05

### Runtime

- Stabilize macOS broker recovery (`5de7743`)

### CLI

- Harden provider delegation (`e865fac`)

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
