# Ponytail 5.1.0 audit

Audit for the Prodex `0.437.0` release prerequisite. No upstream hook or
installer ran during this audit.

Audited at: `2026-10-09T01:27:44Z`

## Official release evidence

- Source: `https://github.com/DietrichGebert/ponytail`
- Release tag: `v5.1.0`
- Tag ref object: `9cc65d03aa2da1db7121b912d03596409ee340b8`
- Commit: `9cc65d03aa2da1db7121b912d03596409ee340b8`
- Commit Git tree: `42320b8382d3bd9eb59521e6b7f2735edcd9b649`
- Source archive: `https://api.github.com/repos/DietrichGebert/ponytail/tarball/v5.1.0`
- Source archive SHA-256: `b460dbaff20a7e366ff597477d6ddfd4ea8f3454c5780e6b6003d5a3b625db6a`

The tag is a direct commit ref. GitHub release metadata marks `v5.1.0` as
non-draft and non-prerelease, published `2026-10-08T16:20:18Z`.

## Prodex tree evidence

The digest uses `crates/prodex-optional-tools/src/tree.rs::tree_sha256` with
domain `prodex-ponytail-tree-v1\0`: sorted normalized relative paths, regular
file lengths as big-endian `u64`, and file bytes. `.git` directories and the
generated `prodex-tool.json` manifest are excluded; symlinks and unsupported
entries are rejected.

- Prodex tree SHA-256: `40281142115c85272f3a3cf6fb4fbd357aea38d6a5be434fee5fe40fad22240c`
- Regular files: `199`
- Total bytes: `10530987`
- Symlinks: `0`
- Tree/file limits: pass

The previous `v5.0.0` qualification remains historically represented by
commit `b088b2df6e08d4306c6a3c3d575fe38c2d2d2989`, Git tree
`cd94f9b4abb6ff6e65055b0076e82a5b13c693aa`, source archive SHA-256
`a5675290f7dd9979fc694988774d548064a906aa1d913335c276a7566a77b718`, and
Prodex tree SHA-256
`4d8896cca8c8214ac8226bb77b8c3572822ba76dcfd8bfd73c70b9dee59f72f7`.

## Compatibility audit

The v5.0.0 to v5.1.0 source comparison showed these relevant manifest,
plugin, hook, and skill paths changed:

- `.claude-plugin/plugin.json`
- `.codex-plugin/plugin.json`
- `hooks/ponytail-instructions.js`
- `skills/ponytail-audit/SKILL.md`
- `skills/ponytail-debt/SKILL.md`
- `skills/ponytail-help/SKILL.md`
- `skills/ponytail-review/SKILL.md`
- `skills/ponytail/SKILL.md`

Both plugin manifests still identify `ponytail` and report `5.1.0`. The Codex
manifest keeps `./skills/` and `./hooks/claude-codex-hooks.json`; the required
hook file remains present. The required `skills/ponytail/SKILL.md` remains
present, and the complete tree contains 13 `skills/` entries. Hook and skill
changes are instruction text changes; required paths remain compatible.

`PONYTAIL_MINIMUM_SUPPORTED_VERSION` remains `4.9.0`. The vetted 4.10.0
commit, tree digest, and legacy manifest digest in `lib.rs` remain unchanged.
