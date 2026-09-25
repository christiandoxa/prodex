# OpenAI tunnel-client v0.0.15 qualification

Release target: Prodex 0.432.0

## Upstream identity

- Repository: https://github.com/openai/tunnel-client
- Release: `v0.0.15`
- Published: `2026-09-25T04:40:41Z`
- Annotated tag object: `f34e8247f17fc964deeac25aead143f6640224cc`
- Peeled release commit: `a390c168ff1b2d14e73a95991c186c6aba3ff5a0`
- Git tree: `ad71a49f78056af76141ed0144e8ff454f382142`
- Codeload tagged-source SHA-256:
  `b14d3404a9029c96c67d371096379d63fd59dcb41870ecfdb64e0242d22ae844`

## Release artifact evidence

Official `tunnel-client-v0.0.15-linux-amd64.zip`:

- GitHub release digest / locally recomputed SHA-256:
  `8c836dc5d68d68b663d9a5c5b28ff9fa780d9f7a3fffb1c306880b8f32fab5f1`
- `--version`:
  `0.0.15+a390c168ff1b2d14e73a95991c186c6aba3ff5a0 (git sha: a390c168ff1b2d14e73a95991c186c6aba3ff5a0)`
- `run --help` exits successfully and retains every Prodex-consumed flag:
  `--config`, `--control-plane.base-url`, `--control-plane.tunnel-id`,
  `--control-plane.api-key`, `--health.listen-addr`,
  `--health.url-file`, `--log.level`, and `--log.format`.

The upstream `cmd/client/root_command.go` still registers `run`. The
v0.0.15 `run_command.go` delta adds embedded stateless/Unix-socket demo
options but does not remove or rename the Prodex-consumed runtime flags.

## Prodex decision

`QUALIFY_COMPATIBLE`

Prodex continues to require stable official-version metadata at or above
`0.0.13` plus a successful `run --help` capability probe. v0.0.15 becomes
the release-qualified latest-stable reference only. No global trust bypass,
credential-source expansion, network-policy relaxation, process fallback, or
lifecycle ownership change is introduced.
