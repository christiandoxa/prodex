# Optional Tools

Prodex discovers optional tools without modifying them. Resolution and health
checks happen before launch; activation writes only to the temporary launch
overlay. Normal launches do not download, clone, update, trust, or grant extra
permissions to a tool.

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

Caveman is not embedded in Prodex. Prodex 0.346.0 accepts the vetted external
release below:

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

For one compatibility release, `<managed-root>/caveman/` is also accepted when
it contains the same manifest and vetted tree. Move it to the versioned path;
the unversioned fallback is scheduled for removal in 0.348.0.

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

Tool selection is independent of provider, Smart Context, workspace trust,
sandbox, and approvals. `prodex super` does not add dangerous bypass flags or
mark a workspace trusted. `--full-access` is the only launch option that asks
Codex for its sandbox bypass; workspace trust remains user-managed.

The compatibility aliases `prodex caveman`, `prodex rtk`, `prodex playwright`,
`prodex ponytail`, and `prodex s` translate to typed tool selections. Legacy
leading tool words are accepted for one release only. Tool-like words later in
the Codex argument list, including `presidio`, are passed through unchanged.
