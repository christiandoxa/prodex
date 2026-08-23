# Optional Tools

Prodex discovers optional tools without modifying them. Normal interactive
launches use bounded path/root resolution and activate only the temporary
overlay; version, daemon, and package health checks remain available through
`prodex capability super-doctor` and are still required for `--require-tool`.
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
prodex capability super-doctor
prodex capability super-doctor --json
```

Missing optional tools do not fail the general doctor or `prodex super`. Use a
required set when absence must be fatal:

```bash
prodex super --require-tool caveman
prodex super --require-tool rtk --dry-run
```

## Caveman

Caveman is not embedded in Prodex. The current source accepts the vetted
external release below:

| Field | Required value |
| --- | --- |
| Version | `2.2.0` |
| Source | `https://github.com/JuliusBrussee/caveman` |
| Commit | `9aa63945a349bef17206540650db48c30fafbdf2` |
| Prodex tree SHA-256 | `91b4549bf361b2aed5ff0d131062788a8c672a941efe9e3db41beecb24a4112a` |

Install the exact checked-out tree at:

```text
<managed-root>/caveman/2.2.0/
```

The directory must contain the upstream `AGENTS.md`,
`skills/caveman/SKILL.md`, `.claude-plugin/plugin.json`, and this strict
manifest as `prodex-tool.json`:

```json
{
  "schema_version": 1,
  "id": "caveman",
  "version": "2.2.0",
  "source": "https://github.com/JuliusBrussee/caveman",
  "commit": "9aa63945a349bef17206540650db48c30fafbdf2",
  "tree_sha256": "91b4549bf361b2aed5ff0d131062788a8c672a941efe9e3db41beecb24a4112a"
}
```

Fetch and installation are explicit user operations. Prodex only validates the
finished tree. `prodex caveman` fails before the TUI when Caveman is missing or
invalid. `prodex super` skips it unless `--require-tool caveman` is present.
`prodex claude caveman` resolves the same installation through its Claude plugin
entry point.

Unversioned managed directories are rejected. Installations must use the exact
versioned path shown above.

## Ponytail

Ponytail uses the same manifest and tree-validation contract at
`<managed-root>/ponytail/4.9.0/`. Its vetted metadata is:

- source: `https://github.com/DietrichGebert/ponytail`
- commit: `0a4dd63ad4541f4f655c4108a295916f3c1d8fda`
- tree SHA-256: `88c6dfa10bc0a63385a8f3f01bc4a3e51963c8fd76a0ebc0426bd889f0705970`

RTK and Codebase Memory MCP resolve from managed roots first and then `PATH`.
The README installs the current stable Codebase Memory MCP `0.10.8`; Prodex
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
`@playwright/mcp@0.0.79` package to pass an offline probe; install the package
and browser before launching Super, then use `prodex capability super-doctor`
to verify it explicitly.
Presidio remains an explicit service selection and is checked by its existing
doctor path. `--require-tool presidio` additionally requires healthy services
and `fail_mode = "closed"`, so an inspection failure cannot silently bypass
redaction.

Native Gemini, Copilot, Kiro, and Antigravity frontends do not consume Codex
overlays, so they reject `--tool` and `--require-tool` instead of claiming the
Codex-only optimizer stack is active. Native Copilot remains the exception for
explicit Presidio redaction through its local provider bridge.

## Security And Launch Semantics

Tool selection remains independent of provider and permissions for the
individual `caveman`, `rtk`, `playwright`, and `ponytail` commands. The
`prodex s` / `prodex super` shortcut is intentionally different: it is the YOLO
entrypoint, adds Codex's approval/sandbox and hook-trust bypass flags, and marks
the current workspace trusted only for that invocation. It does not persist the
trust override. Interactive launches ask about Presidio unless `--presidio` or
`--no-presidio` supplies the choice.

The aliases `prodex caveman`, `prodex rtk`, `prodex playwright`, `prodex
ponytail`, and `prodex s` translate to typed tool selections. Tool-like words
in the Codex argument list are passed through unchanged.
