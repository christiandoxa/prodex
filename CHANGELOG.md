# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.12 - 2026-10-14

### Misc

- Keep saved session IDs readable after Super exits (`f35210d`)
# Prodex 0.437.12

## New Features

No new flags or changes to saved-session storage are required.

## Bug Fixes

### Readable session ID and resume command after exit

After an interactive Super session exits, Prodex now appends a plain-text
footer instead of opening another inline Ratatui panel over the native
Codex usage and session summary. The previous panel could overwrite existing
characters while leaving stale text in blank cells, obscuring the session ID.

The footer prints the complete canonical session ID and a copyable
`prodex s <session-id>` command on separate lines. It is appended with explicit
line boundaries, with no cursor-position query, cursor movement, screen clear,
software width truncation, or reference to a deleted overlay socket as a
working resume endpoint. The footer appears only after the final child exit,
not between automatic relaunch attempts, and is excluded from piped/non-TTY
execution.

Session identity comes from this launch's root-session notification. Fresh
Super sessions still record that identity when automatic rotation is disabled
or no rotation monitor exists. Existing user notification configuration is
preserved. An explicit validated resume ID can be used when no hook reports
an ID; otherwise the footer offers the session picker without guessing from
another terminal's most recently saved session.

Saved conversations, attachments, and paused goals are not changed. Only the
launch-owned marker, offset, lock, and overlay are cleaned after their final
consumer. Paused goals remain paused until `/goal resume`.

## Changelog

The formatter is compiled Mojo with bounded output and UUID validation;
Rust retains terminal and filesystem operations. Regression tests cover
exact session IDs, append-only output, unknown/invalid identities, root-marker
precedence, marker cleanup, broken pipes, and record-only tracking without
auto-rotation. Existing child teardown, terminal-mode restoration and queued
input preservation tests remain in place. The prior runtime-doctor selection
ABI and route-reason tests are separated into focused files to satisfy
existing source-size and Clippy gates without removing tests or relaxing guards.

Full Changelog: [0.437.11...0.437.12](https://github.com/christiandoxa/prodex/compare/0.437.11...0.437.12)

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

## 0.437.2 - 2026-10-10

### Runtime

- Preserve portable affinity recovery and release validation (`a5b927b`)
- Keep quota recovery reachable across busy accounts (`57a1ee2`)
- Move compact continuation policy to Mojo (`5f91526`)
- Exhaust websocket attempts correctly across recovery sweeps (`d191200`)
- Map Mojo affinity policy errors at app boundary (`d7cc909`)
- Migrate runtime proxy body limit policy (`5357e12`)

### CLI

- Preserve turn-state owner facts in quota replay eligibility (`77ffdf0`)

### Docs

- Qualify Codex rust-v0.162.1 (`d59ab48`)

### Misc

- Preserve DeepSeek usage output and metadata edge cases (`f71e135`)
- Preserve Gemini data URL and media text contracts (`0b84d7a`)
- Select oldest cookie across bounded ABI chunks without Rust fallback (`fff216f`)

## 0.437.1 - 2026-10-09

### Runtime

- Preserve previous-response recovery precedence (`50f0004`)
- Recover headerless and bound-turn upstream capacity (`bcecbb6`)
- Move summary log formatting to Mojo (`e600780`)

### CLI

- Saturate profile geometry at integer boundaries (`75365cc`)
- Migrate profile screen geometry policy (`9e04540`)
