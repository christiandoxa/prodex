# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.14 - 2026-10-15

### Runtime

- Recover authenticated profiles across quota transitions (`c49981d`)

## 0.437.13 - 2026-10-15

### Runtime

- Preserve terminal output and recover observer pressure (`bf2cc2b`)

### Docs

- Record login authority coverage (`980c21d`)
- Record extended Mojo wave (`e6c410a`)
- Record Mojo 75 campaign wave (`8d4e04e`)
- Record websocket frame checkpoint (`693bb73`)
- Record status parser checkpoint (`bbc9646`)

### Deps

- Update rustls for October security advisories (`a2bd78c`)

### Misc

- Audit RTK 1.0.1 release reference (`35f5b76`)
- Migrate broker continuity metrics reducer (`229ec56`)
- Align stream JSON validation with serde (`54235b5`)
- Restore ownership manifest (`1087b5d`)

## 0.437.12 - 2026-10-14

### Misc

- Keep saved session IDs readable after Super exits (`f35210d`)

## 0.437.11 - 2026-10-14

### Runtime

- Persist session release fences across resume and rotation (`da0fabc`)
- Accept trimmed header tuple bounds (`b4dea3f`)
- Migrate marker aggregation to Mojo (`d37d0f9`)

### CLI

- Move panel error line normalization to Mojo (`2affd22`)

### Docs

- Record latest migration checkpoints (`5980212`)

### Misc

- Preserve complete marker facets and simplify ABI projection (`610cf97`)

## 0.437.10 - 2026-10-13

### Misc

- Preserve pasted goal media across overlay exit and resume (`0548b3f`)

## 0.437.9 - 2026-10-13

### Runtime

- Integrate native capacity recovery safeguards (`bd3c1d5`)
- Preserve root session recovery ownership and cursor (`bca7e83`)
- Keep Responses heartbeats uncommitted (`6e49493`)

## 0.437.8 - 2026-10-13

### Runtime

- Bound recovery waits without reopening live sessions (`ef7cf36`)
- Recover native request timeouts after compaction (`cc146bb`)
- Plan runtime route circuit actions (`dec6d74`)
- Migrate websocket recovery predicates (`3421a40`)

### Deps

- Align Kiro stable native release probe (`082bfc9`)
- Verify Ponytail 5.2.0 compaction compatibility (`38112e8`)
- Audit Ponytail 5.1.1 stable source (`ef94261`)
- Update quinn-proto past peer-triggered panic advisories (`f7e2028`)

### Misc

- Migrate Gemini thought signatures (`eacb848`)

## 0.437.7 - 2026-10-12

### Runtime

- Recover terminal native threads without relaunch (`c554fc0`)
- Preserve Codex context defaults and recover native threads in place (`82d311f`)

## 0.437.6 - 2026-10-12

### Runtime

- Keep large-session recovery armed after compaction (`7151012`)

### Deps

- Bump rust from 1.98.1-bookworm to 1.99.0-bookworm (`db5cb81`)
- Bump the cargo group with 6 updates (`ea659d2`)

### Misc

- Use Rust 1.99 atomic update API (`c757a6a`)
- Merge Dependabot PR #111 for 0.437.6 (`4ac9273`)
- Merge Dependabot PR #110 for 0.437.6 (`02eb770`)
- Merge Dependabot PR #109 for 0.437.6 (`0164486`)

## 0.437.5 - 2026-10-11

### Runtime

- Stop wrapped native TUI before retiring its endpoint (`1fe2262`)

### CLI

- Preserve long activation profile names (`386db91`)

### Misc

- Repair native goal attachment aliases before resume (`494c414`)

## 0.437.4 - 2026-10-11

### Runtime

- Reset recovery evidence on native task start (`886bcc5`)
- Recover native Codex task completion failures (`96cf9ab`)

## 0.437.3 - 2026-10-10

### Runtime

- Reap the session-owned native app-server process tree (`2efed0b`)
- Prevent admission and quota-state notification deadlock (`370ee38`)
