# Optional Tools

Prodex discovers optional tools without modifying them. Normal interactive
launches use bounded path/root resolution and activate only the temporary
overlay; version, daemon, and package health checks remain available through
`prodex doctor --install` and are still required for `--require-tool`.
Normal launches do not download, clone, update, trust, or grant extra
permissions to a tool.

Do not replace an externally managed optional-tool binary while its daemon or
clients are still active. Finish those sessions before updating it; if a mixed
binary cohort still fails at startup, Prodex preserves the tool's bounded stderr
diagnostic instead of reporting only the child exit status.

## Discovery

Managed roots are searched in this order:

1. `PRODEX_OPTIMIZERS_HOME`
2. `$XDG_DATA_HOME/prodex-optimizers`
3. `$HOME/.local/share/prodex-optimizers`, or
   `%USERPROFILE%\.local\share\prodex-optimizers` on Windows when `HOME` is unset

Command tools may also be resolved from `PATH`. Managed paths are canonicalized;
paths escaping a managed root, symlinks in plugin trees, unsupported file types,
oversized files, oversized trees, and invalid manifests are rejected.

Run this bounded, offline check:

```bash
prodex doctor --install
```

Missing optional tools do not fail the general doctor or `prodex super`. Use a
required set when absence must be fatal:

```bash
prodex super --require-tool caveman
prodex super --require-tool rtk --dry-run
```

Release maintainers can recheck official stable versions with
`npm run optional-tools:freshness`. The checked-in inventory at
`migration/optional-tools-audit.json` records the release-time source revisions,
asset checksums, trust model, and cross-platform validation decisions. Normal
Prodex launches never run this online check or update an external tool.

## Caveman

Caveman is not embedded in Prodex. The current source accepts the vetted
external release below:

| Field | Required value |
| --- | --- |
| Version | `2.7.0` |
| Source | `https://github.com/JuliusBrussee/caveman` |
| Commit | `8b0c1d3699b8d83e87fe4605b378da20c41555e0` |
| Prodex tree SHA-256 | `26d587fc179e79f76f4e2b42edec0266a7af40cf08bf15eb4609de310fabd8fb` |

Install the exact checked-out tree at:

```text
<managed-root>/caveman/2.7.0/
```

The directory must contain the upstream `AGENTS.md`,
`skills/caveman/SKILL.md`, `.claude-plugin/plugin.json`, and this strict
manifest as `prodex-tool.json`:

```json
{
  "schema_version": 1,
  "id": "caveman",
  "version": "2.7.0",
  "source": "https://github.com/JuliusBrussee/caveman",
  "commit": "8b0c1d3699b8d83e87fe4605b378da20c41555e0",
  "tree_sha256": "26d587fc179e79f76f4e2b42edec0266a7af40cf08bf15eb4609de310fabd8fb"
}
```

Fetch and installation are explicit user operations. Prodex only validates the
finished tree. Standalone optimizer and Claude-plugin command paths are retired;
`prodex super` skips an optional tool unless `--require-tool <tool>` is present.

Unversioned managed directories are rejected. Installations must use the exact
versioned path shown above.

## Ponytail

Ponytail uses the same manifest and tree-validation contract at
`<managed-root>/ponytail/4.10.0/`. Its vetted metadata is:

- source: `https://github.com/DietrichGebert/ponytail`
- commit: `1d95ff7d39de12d87014ea40d4e22201bddc501b`
- tree SHA-256: `5443a5ee4a7248adcb59e1e102dd5bbd14af3083a9c3b4f271dd86790ac88c9c`

RTK `0.49.0` is the latest stable release validated for this Prodex release; it remains
externally managed and version-compatible rather than latest-only. Codebase Memory MCP
`0.11.0` is the latest stable release validated for this Prodex release. Both resolve from
managed roots first and then `PATH`.
The README installs the current stable Codebase Memory MCP `0.11.0`; Prodex
continues to accept `0.9.1-rc.1` or newer (or a development build) and
expose its native `daemon status` contract. The explicit health check verifies
this contract; normal optional launch resolution does not synchronously spawn
the daemon probe. Parallel Codex processes retain
their own lightweight stdio frontends, while the daemon shares indexing jobs,
watchers, and the graph cache; legacy builds that duplicate heavy per-session
work fail the health check. Prodex leaves `CBM_CACHE_DIR` unset so parent and
sub-agent sessions join the canonical account daemon, while an explicit user
override is inherited unchanged.
Kiro launches retain that shared server but add `check_index_coverage` to the
server's `disabledTools` list because Kiro/Bedrock rejects its top-level JSON
Schema composition; all other Codebase Memory tools remain available.
Playwright MCP requires validated Node.js 18+, `npx`, and the pinned
`@playwright/mcp@0.0.82` package to pass an offline probe; install the package
and browser before launching Super, then use `prodex doctor --install`
to verify it explicitly.
Presidio remains an explicit service selection and is checked by its existing
doctor path. `--require-tool presidio` additionally requires healthy services
and `fail_mode = "closed"`, so an inspection failure cannot silently bypass
redaction. Presidio 2.2.364 currently constrains `cryptography` below the
50.0.0 fix for
[GHSA-g6cj-pr64-35w5](https://github.com/advisories/GHSA-g6cj-pr64-35w5).
The affected PKCS#7 EnvelopedData decrypt
API is not used by Prodex's tested anonymize route, but the optional image is
not reported as dependency-clean while
[upstream issue #2229](https://github.com/data-privacy-stack/presidio/issues/2229)
remains open.

Native Gemini, Copilot, Kiro, and Antigravity frontends do not consume Codex
overlays, so they reject `--tool` and `--require-tool` instead of claiming the
Codex-only optimizer stack is active. Native Copilot remains the exception for
explicit Presidio redaction through its local provider bridge.

## Security And Launch Semantics

Tool selection remains independent of provider and permissions for the
individual typed Super tool flags. The
`prodex s` / `prodex super` shortcut is intentionally different: it is the YOLO
entrypoint, adds Codex's approval/sandbox and hook-trust bypass flags, and marks
the current workspace trusted only for that invocation. It does not persist the
trust override. Interactive launches ask about Presidio unless `--presidio` or
`--no-presidio` supplies the choice.

The aliases `prodex caveman`, `prodex rtk`, `prodex playwright`, and `prodex
ponytail` are retired. Select tools through `prodex super --tool <tool>` or
`--require-tool <tool>`; tool-like words in the Codex argument list are passed
through unchanged.
