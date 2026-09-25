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
Prodex launches never run this online check or update an external tool. Runtime compatibility is minimum/capability-based: missing tools remain optional, compatible stable releases at or above the minimum are accepted, and an installed-but-incompatible tool fails with an upgrade instruction. The audited latest-stable versions below are release-qualified references, not exact runtime allowlists.

## Caveman

Caveman is not embedded in Prodex. Runtime accepts stable Caveman 2.3.1 or newer from a validated managed version directory. The table below records the current latest-stable release-qualified reference:

| Field | 0.431.5 qualified reference |
| --- | --- |
| Version | `2.7.0` |
| Source | `https://github.com/JuliusBrussee/caveman` |
| Commit | `8b0c1d3699b8d83e87fe4605b378da20c41555e0` |
| Prodex tree SHA-256 | `09127915a13a493146ed0392b6895bbbd5f620d276dda9f4e68a6722f96df950` |

For the current release-qualified reference, the managed path is:

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
  "tree_sha256": "09127915a13a493146ed0392b6895bbbd5f620d276dda9f4e68a6722f96df950"
}
```

Fetch and installation are explicit user operations. Prodex only validates the
finished tree. Standalone optimizer and Claude-plugin command paths are retired;
`prodex super` skips an optional tool unless `--require-tool <tool>` is present.

Unversioned managed directories are rejected. Prodex chooses the newest stable version directory at or above 2.3.1. A newer stable release is accepted when the official source, manifest schema, required files, commit shape, and recomputed tree digest are self-consistent; the current latest-stable reference keeps the stronger audited commit/tree check.

Prodex recomputes the complete tree digest before activation and treats that
recomputed digest as authoritative. Prodex 0.430.3 also accepts the legacy
prodex-tool.json digest written by earlier installers for the current release-qualified version and commit, so a clean official checkout is not rejected solely
because its manifest was generated with stale digest metadata. The actual tree
must still match the current vetted digest above.

## Ponytail

Ponytail uses the same versioned manifest/tree contract, accepts stable 4.9.0 or newer, and selects the newest compatible managed directory. The current release-qualified reference is `<managed-root>/ponytail/4.10.0/` with metadata:

- source: `https://github.com/DietrichGebert/ponytail`
- commit: `1d95ff7d39de12d87014ea40d4e22201bddc501b`
- tree SHA-256: `05fe532f2a310cc7d12a60d6b51d1638a7d1465598d82ffa9f6a3d4cbf970f48`

The same legacy-manifest compatibility rule applies to the 4.10.0 qualified reference. Future stable releases are accepted when the plugin version matches the managed manifest and the recomputed tree digest matches that manifest.

RTK requires `0.46.0` or newer; `0.50.0` is the latest stable release-qualified reference for this Prodex release. It remains externally managed and version-compatible rather than latest-only. Codebase Memory MCP
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
Playwright MCP requires validated Node.js 18+, `npx`, and Playwright MCP `0.0.79` or newer. Prodex invokes `npx --no-install @playwright/mcp`, probes the installed package version, and rejects an installed version below the minimum. `0.0.82` is the current latest-stable release-qualified reference.
Presidio remains an explicit service selection and is checked by its existing doctor path. Prodex accepts official Presidio analyzer/anonymizer images at `2.2.364` or newer; the release-qualified defaults remain digest-pinned, while newer official images can be supplied with `PRODEX_PRESIDIO_ANALYZER_IMAGE` and `PRODEX_PRESIDIO_ANONYMIZER_IMAGE`. `--require-tool presidio` additionally requires healthy services
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
entrypoint, applies Codex approval/sandbox overrides, marks the current workspace trusted only for that invocation, and pre-trusts discovered hooks by their exact Codex hook hash. It does not use the global dangerous hook-bypass flag and does not persist the workspace trust override. Interactive launches ask about Presidio unless `--presidio` or
`--no-presidio` supplies the choice.

The aliases `prodex caveman`, `prodex rtk`, `prodex playwright`, and `prodex
ponytail` are retired. Select tools through `prodex super --tool <tool>` or
`--require-tool <tool>`; tool-like words in the Codex argument list are passed
through unchanged.
