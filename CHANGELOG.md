# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.274.0 - 2026-07-11

### Runtime

- Add async redis limiter runtime (`57a7e3e`)
- Add pooled postgres runtime (`df800b4`)
- Harden enterprise gateway runtime (`08e2200`)
- Export config publication runtime delivery adapter (`3c3211b`)
- Clean npm smoke and runtime policy test-only helpers (`c98918f`)

### CLI

- Rotate gateway credentials atomically (`1a070dd`)
- Pin projected secret generation (`6074230`)
- Add development provider (`926ba22`)
- Wire production gateway references (`d30bd49`)
- Add projected external provider (`492fd82`)

### Docs

- Align migration job runbook (`89f25a1`)

### Misc

- Enforce postgres transport tls (`147e6e5`)
- Enforce grouped request budgets (`b97ed3d`)
- Enforce distributed rpm tpm (`2c3a02e`)
- Add atomic redis rpm tpm admission (`9ef8c6c`)
- Wire redis coordination reference (`4fd9d5d`)
- Wire pooled postgres accounting (`34f3d54`)
- Wire dedicated async serve (`95bbd73`)
- Add async compatibility front (`9184705`)
- Drain on shutdown signals (`20d736a`)
- Project gateway secrets (`2981d57`)
- Make postgres rls migration repeatable (`29a120f`)
- Preserve last-known-good on reload failure (`39e69af`)
- Mature provider gateway and control plane (`cef3d8d`)
- Validate legacy admin ledger pagination queries (`fc5ba2b`)
- Delegate admin governance headers to shared boundary (`c2d5028`)
- Add control-plane http planning command (`b8a5147`)
- Add control-plane publication planning command (`d09a5c0`)
- Add control-plane publication delivery command (`c8253c3`)
- Unblock local preflight and provider bridge tests (`aa74651`)
- Checkpoint enterprise readiness refactor (`71e4274`)

## 0.273.0 - 2026-07-10

### CLI

- Skip workspace trust prompt (`0c2e464`)

## 0.272.0 - 2026-07-10

### Runtime

- Harden gateway and proxy boundaries (`1fc05d4`)

### CLI

- Simplify optimizer candidate lookup (`11ebc21`)

## 0.270.0 - 2026-07-10

### CLI

- Trust launch directory in yolo mode (`436bce1`)

## 0.269.0 - 2026-07-10

### Misc

- Protect active session transcripts (`f2113d1`)

## 0.268.0 - 2026-07-10

### Runtime

- Harden runtime compatibility and launch handling (`1476ca5`)

## 0.267.0 - 2026-07-09

### Runtime

- Harden auth and gateway enforcement (`c457bfe`)

## 0.266.0 - 2026-07-09

### Misc

- Connect stacked panel borders (`7e0c16f`)

## 0.265.0 - 2026-07-08

### CLI

- Simplify log quota header (`5bf9567`)

## 0.264.0 - 2026-07-08

### CLI

- Smooth adaptive log quota header (`33273cd`)

## 0.263.0 - 2026-07-08

### CLI

- Marquee log quota header (`c5cc036`)

### Misc

- Update upstream compatibility watchdog (`b5db7c3`)

## 0.262.0 - 2026-07-08

### CLI

- Show profile quota in log headers (`5719800`)

### Misc

- Satisfy log TUI clippy gate (`559f856`)
