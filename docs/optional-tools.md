# Optional Tools

Prodex discovers optional tools without modifying them. Resolution and health
checks happen before launch; activation writes only to the temporary launch
overlay. Normal launches do not download, clone, update, trust, or grant extra
permissions to a tool.

Do not replace an externally managed optional-tool binary while its daemon or
clients are still active. Finish those sessions before updating it; if a mixed
binary cohort still fails at startup, Prodex preserves the tool's bounded stderr
diagnostic instead of reporting only the child exit status.

## Discovery

Managed roots are searched in this order:

1. `PRODEX_OPTIMIZERS_HOME`
2. `$XDG_DATA_HOME/prodex-optimizers`
3. the platform user-data fallback, such as
   `$HOME/.local/share/prodex-optimizers` on Linux

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
| Version | `1.9.1` |
| Source | `https://github.com/JuliusBrussee/caveman` |
| Commit | `0d95a81d35a9f2d123a5e9430d1cfc43d55f1bb0` |
| Prodex tree SHA-256 | `863d1a6965ed47f9e130312c8e943617e224cc08f8162296d7e06b8b63d54476` |

Install the exact checked-out tree at:

```text
<managed-root>/caveman/1.9.1/
```

The directory must contain the upstream `AGENTS.md`,
`skills/caveman/SKILL.md`, `.claude-plugin/plugin.json`, and this strict
manifest as `prodex-tool.json`:

```json
{
  "schema_version": 1,
  "id": "caveman",
  "version": "1.9.1",
  "source": "https://github.com/JuliusBrussee/caveman",
  "commit": "0d95a81d35a9f2d123a5e9430d1cfc43d55f1bb0",
  "tree_sha256": "863d1a6965ed47f9e130312c8e943617e224cc08f8162296d7e06b8b63d54476"
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
`<managed-root>/ponytail/4.8.4/`. Its vetted metadata is:

- source: `https://github.com/DietrichGebert/ponytail`
- commit: `16f29800fd2681bdf24f3eb4ccffe38be3baec6b`
- tree SHA-256: `727ac132ab903b3abf46cabd3d8ee855984e83d6f8ef36665853604c9a5c2e7d`

RTK and Codebase Memory MCP resolve from managed roots first and then `PATH`.
Playwright MCP requires validated Node.js 18+ and `npx`. Presidio remains an
explicit service selection and is checked by its existing doctor path.

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
