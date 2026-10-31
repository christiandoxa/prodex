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

Command tools may also be resolved from `PATH`. Managed executable paths are canonicalized and confined before any version or
health probe executes;
paths escaping a managed root, symlinks in plugin trees, unsupported file types,
oversized files, oversized trees, and invalid manifests are rejected.

The install doctor checks all four automatically selected tools, including their
version, shared-daemon, package, or plugin-tree contracts as appropriate. Run this
bounded local check:

```bash
prodex doctor --install
```

Missing optional tools do not fail the general doctor or `prodex super`. Use a
required set when absence must be fatal:

```bash
prodex super --require-tool ponytail
prodex super --require-tool rtk --dry-run
```

Release maintainers can recheck official stable versions with
`npm run optional-tools:freshness`. The checked-in inventory at
`migration/optional-tools-audit.json` records the release-time source revisions,
asset checksums, trust model, and cross-platform validation decisions. Normal
Prodex launches never run this online check or update an external tool. Runtime compatibility is minimum/capability-based: missing tools remain optional, compatible stable releases at or above the minimum are accepted, and an installed-but-incompatible tool fails with an upgrade instruction. The audited latest-stable versions below are release-qualified references, not exact runtime allowlists.

## Automatic Super stack

`prodex s` and `prodex super` automatically select RTK, Codebase Memory MCP,
Playwright MCP, and Ponytail. They do not require a `--tool` flag. Existing
explicit user MCP configuration remains authoritative; absent default entries
are created in the temporary overlay. Presidio still asks for consent before
an interactive launch; `--presidio` and `--no-presidio` keep their existing
non-interactive meanings. `--tool` adds to the automatic set, not replaces it.

The release-qualified references for 0.166.1 are RTK 1.1.0, Codebase Memory MCP
0.12.1, Playwright MCP 0.0.83, Ponytail 5.3.3, and Presidio 2.2.366. The freshness
gate verifies upstream GitHub releases and the applicable npm/PyPI metadata.
Qualification does not automatically replace running tools or services.

## Retired Caveman integration

Caveman is no longer an optional tool as of 0.166.1. Explicit `--tool caveman`,
`--require-tool caveman`, and `optional:caveman` are rejected. The Super overlay
removes old Prodex Caveman marketplace entries, exact legacy instruction
paragraphs, and managed versioned `AGENTS.md` references. User-authored
instructions and the base Codex home remain unchanged. Externally installed
Caveman files are not uninstalled or modified by Prodex.

Restart an existing expose endpoint after upgrading to refresh its startup-time
optional-tool inventory. Retired ABI id 0 remains reserved rather than being
reused for another tool.

## Ponytail

Ponytail uses a versioned manifest/tree contract, accepts stable 4.9.0 or newer, and selects the newest compatible managed directory. The current release-qualified reference is `<managed-root>/ponytail/5.3.3/` with metadata:

- source: `https://github.com/DietrichGebert/ponytail`
- commit: `7801d061243d7b753bad0e0b3c057cd250b11755`
- tree SHA-256: `3bd59937f9375a418f9f85c92b30b5d1404491578b784afa76d32ab2c1173af3`

The 5.3.3 and previously qualified 5.3.2 references require their exact audited commits and complete tree digests. Existing stable 4.10.x checkouts remain accepted through the minimum-version and manifest-digest compatibility rules, including the previously qualified 4.10.0 legacy-manifest rule. Future stable releases are accepted when the plugin version matches the managed manifest and the recomputed tree digest matches that manifest.

RTK requires `0.46.0` or newer; `1.1.0` is the latest stable release-qualified reference for this Prodex release. It remains externally managed and version-compatible rather than latest-only. Codebase Memory MCP
`0.12.1` is the latest stable release validated for this Prodex release. Both resolve from
managed roots first and then `PATH`.
The README installs the current stable Codebase Memory MCP `0.12.1`; Prodex
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
Both ordinary launch and required-tool checks verify the Node runtime before
probing the installed Playwright package. Playwright MCP requires validated Node.js 18+, `npx`, and Playwright MCP `0.0.79` or newer. Prodex invokes `npx --no-install @playwright/mcp`, probes the installed package version, and rejects an installed version below the minimum. `0.0.83` is the current latest-stable release-qualified reference.
Presidio remains a consent-controlled service selection and uses the existing
launch/expose health checks. Prodex accepts official Presidio analyzer/anonymizer images at `2.2.364` or newer; the release-qualified defaults remain digest-pinned, while newer official images can be supplied with `PRODEX_PRESIDIO_ANALYZER_IMAGE` and `PRODEX_PRESIDIO_ANONYMIZER_IMAGE`. `--require-tool presidio` additionally requires healthy services
and `fail_mode = "closed"`, so an inspection failure cannot silently bypass
redaction. The release-qualified 2.2.366 defaults are pinned to official image-index
SHA-256 digests. Isolated service probes verify email detection, replacement and
health responses without exposing host ports or using user data. The anonymizer
image contains `cryptography` 50.0.2, outside the previously documented
GHSA-g6cj-pr64-35w5 affected range; this is not a blanket clean-image claim.
Existing compatible 2.2.364 deployments are not automatically replaced, and
English images still do not imply Indonesian recognizer support.

Native Gemini, Copilot, Kiro, and Antigravity frontends do not consume Codex
overlays, so they reject `--tool` and `--require-tool` instead of claiming the
Codex-only optimizer stack is active. Native Copilot remains the exception for
explicit Presidio redaction through its local provider bridge.

## Security And Launch Semantics

Tool selection remains independent of provider and permissions for the
individual typed Super tool flags. The
`prodex s` / `prodex super` shortcut is intentionally different: it is the YOLO
entrypoint, applies Codex approval/sandbox overrides, marks the launch workspace
trusted only for that invocation, and also pre-trusts canonical persisted session
workspaces needed by direct UUID resume and Codex `/resume`. It pre-trusts
discovered hooks by their exact Codex hook hash. It does not use the global
dangerous hook-bypass flag and does not persist workspace trust. Interactive
launches ask about Presidio unless `--presidio` or `--no-presidio` supplies the
choice.

The aliases `prodex caveman`, `prodex rtk`, `prodex playwright`, and `prodex
ponytail` are retired. Select tools through `prodex super --tool <tool>` or
`--require-tool <tool>`; tool-like words in the Codex argument list are passed
through unchanged.

RTK 1.0.0 ships an Apache-2.0-only `rtk-oss` edition; its default full binary
also links a BUSL-1.1 engine. Prodex does not auto-install either edition or
change the supported minimum; administrators choose their externally managed
RTK distribution. The `proxy` command remains a compatible alias of `bypass`.

The 0.12.0 reference was refreshed for the 0.438.0 release. Its first later
indexing run rebuilds an older project index according to upstream upgrade
notes; release qualification does not trigger that rebuild or replace an
installed shared daemon.

The 0.438.1 freshness qualification updates Codebase Memory MCP to 0.12.1 and
Ponytail to 5.3.2. All six Codebase Memory platform archives were downloaded
and matched against upstream checksums and GitHub asset digests. Ponytail's
complete source tree matched the official Git blobs and the bounded Prodex
tree digest. The read-only daemon health probe can see an existing per-account
daemon even with an isolated HOME; qualification did not restart that daemon,
rebuild user indexes, run Ponytail hooks, or import past sessions.
