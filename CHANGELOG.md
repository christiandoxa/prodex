# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.213.0 - Unreleased

Changes after `0.211.0`.

### Runtime

- Stabilize history image attachment paths after child exit (`17b45be`)

### CLI

- Extract provider flags from codex_args for session-id-first syntax (`f1783cd`)

## 0.211.0 - 2026-06-24

### Runtime

- Harden runtime rewrites (`0902526`)

### Docs

- Document smart context command flow (`052db22`)

### Misc

- Tune rewrite policy accounting (`b3f5a94`)
- Add replay metrics scaffold (`e81331f`)
- Add candidate rollout primitives (`6a0d685`)
- Align provider transport metadata (`e76e603`)

## 0.210.0 - 2026-06-24

### Runtime

- Preserve upstream Codex backend metadata (`ebb8be5`)

## 0.209.0 - 2026-06-23

### Misc

- Tolerate successful reset credit response (`db91125`)

## 0.208.0 - 2026-06-23

### Misc

- Guard reset credit redemption (`ef0dd36`)

## 0.207.0 - 2026-06-23

### Runtime

- Add Codex feature override flags (`fdb1a5c`)

### CLI

- Space live controls from detail rows (`ebbab88`)

## 0.206.0 - 2026-06-23

### Misc

- Add reset credit redemption (`247a63c`)

## 0.205.0 - 2026-06-23

### Deps

- Update quinn-proto advisory (`4a6863d`)

## 0.204.0 - 2026-06-22

### Runtime

- Stabilize runtime profile reporting (`e960033`)

## 0.203.0 - 2026-06-22

### Runtime

- Skip websocket prewarm smart context rewrite (`9d1e927`)

### Misc

- Improve stream and upstream rendering (`1157571`)

## 0.202.0 - 2026-06-22

### Deps

- Bump redis from 0.32.7 to 1.2.4 in the cargo group (`7df2cfa`)

### Misc

- Merge pull request #21 from christiandoxa/dependabot/github_actions/github-actions-640176b5ab (`4b809c0`)
- Merge pull request #20 from christiandoxa/dependabot/cargo/cargo-ce48053083 (`3f64fe8`)

## 0.201.0 - 2026-06-21

### CLI

- Keep optimizer state out of worktrees (`3695eaf`)

## 0.200.0 - 2026-06-21

### CLI

- Repair stale active profile selection (`e3c3b69`)
