# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.69.0 - Unreleased

Changes after `0.67.0`.

### Runtime

- Promote fresh websocket rotations (`00dcc41`)
- Make auto-rotate opt-in (`616c8d0`)
- Split runtime support crates (`11bddfb`)

### CLI

- Satisfy quota clippy gate (`fb4265e`)
- Add session inspection and quota auth filtering (`24472a0`)

### CI

- Relax sse bench smoke threshold (`203afbd`)
- Update split crate lint checks (`49536ce`)

### Misc

- Split internal workspace crates (`7881ca4`)
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

## 0.62.0 - 2026-04-28

### Runtime

- Improve diagnostics and release readiness (`cf19a7e`)

### CI

- Avoid synthetic secret scan matches (`68e0398`)
- Refresh upstream watchdog baseline (`4bccb9f`)

## 0.61.0 - 2026-04-28

### Runtime

- Harden proxy and shared state resilience (`439da04`)

## 0.60.0 - 2026-04-28

### Runtime

- Improve runtime diagnostics and import safety (`3c62d3a`)

## 0.59.0 - 2026-04-28

### Runtime

- Improve runtime diagnostics and preflight tooling (`efb9f4e`)

## 0.58.0 - 2026-04-28

### CLI

- Improve prodex super local launch (`dc8e7fe`)

## 0.57.0 - 2026-04-28

### Runtime

- Support proxied runtime broker launches (`76ac1de`)

## 0.56.0 - 2026-04-27

### Runtime

- Satisfy clippy upstream request helper (`965fdb6`)
- Harden websocket pressure and diagnostics (`6b72fde`)

### CLI

- Overwrite tokens on import name match (`03faa4d`)
