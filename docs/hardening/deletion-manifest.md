# Deletion Manifest

Baseline: `c635d485cf750637512bfacb4e1cefc854ed1bef`.

Every file deleted by this hardening series is listed below. Renames are
reported as deletions plus their canonical replacement so history remains
auditable.

## Embedded Caveman package

Deleted because Caveman is now a validated external optional tool. Generic overlay ownership moved to `prodex-optional-tools`; Caveman content remains user-managed.

Evidence: `cargo metadata --locked`, `npm run ci:optional-tools-guard`, and optional-tool integration tests.

- `crates/prodex-caveman-assets/caveman_assets/.codex-plugin/plugin.json`
- `crates/prodex-caveman-assets/caveman_assets/assets/caveman-dark.svg`
- `crates/prodex-caveman-assets/caveman_assets/assets/caveman-small.svg`
- `crates/prodex-caveman-assets/caveman_assets/assets/caveman.svg`
- `crates/prodex-caveman-assets/caveman_assets/claude/.claude-plugin/plugin.json`
- `crates/prodex-caveman-assets/caveman_assets/claude/commands/caveman-commit.toml`
- `crates/prodex-caveman-assets/caveman_assets/claude/commands/caveman-review.toml`
- `crates/prodex-caveman-assets/caveman_assets/claude/commands/caveman.toml`
- `crates/prodex-caveman-assets/caveman_assets/claude/hooks/caveman-activate.js`
- `crates/prodex-caveman-assets/caveman_assets/claude/hooks/caveman-config.js`
- `crates/prodex-caveman-assets/caveman_assets/claude/hooks/caveman-mode-tracker.js`
- `crates/prodex-caveman-assets/caveman_assets/claude/hooks/caveman-statusline.ps1`
- `crates/prodex-caveman-assets/caveman_assets/claude/hooks/caveman-statusline.sh`
- `crates/prodex-caveman-assets/caveman_assets/claude/skills/caveman-commit/SKILL.md`
- `crates/prodex-caveman-assets/caveman_assets/claude/skills/caveman-help/SKILL.md`
- `crates/prodex-caveman-assets/caveman_assets/claude/skills/caveman-review/SKILL.md`
- `crates/prodex-caveman-assets/caveman_assets/skills/caveman/SKILL.md`
- `crates/prodex-caveman-assets/caveman_assets/skills/caveman/agents/openai.yaml`
- `crates/prodex-caveman-assets/caveman_assets/skills/caveman/assets/caveman-small.svg`
- `crates/prodex-caveman-assets/caveman_assets/skills/caveman/assets/caveman.svg`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/SKILL.md`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/__init__.py`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/__main__.py`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/benchmark.py`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/cli.py`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/compress.py`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/detect.py`
- `crates/prodex-caveman-assets/caveman_assets/skills/compress/scripts/validate.py`
- `crates/prodex-caveman-assets/src/asset_verification.rs`
- `crates/prodex-caveman-assets/src/embedded_files.rs`
- `crates/prodex-caveman-assets/src/embedded_tree.rs`
- `crates/prodex-caveman-assets/src/lib.rs`
- `crates/prodex-caveman-assets/src/marketplace.rs`
- `crates/prodex-caveman-assets/tests/src/lib.rs`

## Unsafe or redundant Smart Context passes

Deleted heuristic, alias, manifest, rehydration, and eager-persistence paths replaced by the scoped plan–validate–commit engine and inline-reference validation.

Evidence: Focused `prodex-app` Smart Context tests and the generated deterministic replay report.

- `crates/prodex-app/src/runtime_background/scheduled_save/artifact_save.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/artifact_manifest/aliases.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/intent.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/rehydration.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/rehydration/appendix.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/repo_state.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/repo_state/facts.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/repo_state/parser.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/repo_state/rewrite.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/runtime_rehydrate/appendix.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/runtime_rehydrate/appendix/selection.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/runtime_rehydrate/scoring.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/runtime_rehydrate/selective.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/static_context/items.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/static_context/sections.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/static_observation.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/tool_outputs.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/tool_outputs/arguments.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/tool_outputs/compaction.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/tool_outputs/dedupe.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/tool_outputs/diff.rs`
- `crates/prodex-app/src/runtime_proxy/smart_context/tool_outputs/metadata.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/aliases.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/intent.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/manifest.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/rehydration.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/repo_artifacts.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/semantic.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/static_context_extra/dedupe.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/static_context_extra/delta.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/tool_outputs.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/tool_outputs/arguments.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/tool_outputs/artifacts.rs`
- `crates/prodex-app/tests/src/runtime_proxy/smart_context/tool_outputs/summaries.rs`

## Self-asserted replay aggregation

Deleted metrics-only evaluation/rendering. The production engine now generates per-turn results from an inputs-only corpus.

Evidence: `npm run smart-context:replay` and `npm run docs:smart-context-evidence:check`.

- `crates/prodex-runtime-proxy/src/smart_context/replay/evaluation.rs`
- `crates/prodex-runtime-proxy/src/smart_context/replay/evaluation/acceptance.rs`
- `crates/prodex-runtime-proxy/src/smart_context/replay/markdown.rs`

## Renumbered enterprise governance documents

Old paths had duplicate numeric prefixes. Full content moved to uniquely numbered documents 15–22; the governance document map records each mapping.

Evidence: `npm run docs:lint` and `npm run ci:enterprise-docs-guard`.

- `docs/enterprise-governance/04-classification-and-obligations.md`
- `docs/enterprise-governance/05-response-stream-enforcement.md`
- `docs/enterprise-governance/07-policy-approval-and-store.md`
- `docs/enterprise-governance/08-audit-siem-and-evidence.md`
- `docs/enterprise-governance/10-unified-gateway-and-identity.md`
- `docs/enterprise-governance/11-operations-slos-and-alerts.md`
- `docs/enterprise-governance/12-testing-performance-and-evidence.md`
- `docs/enterprise-governance/13-rollout-rollback-and-deprecation.md`

## One-off documentation patch scripts

Deleted unreferenced one-off patch scripts after README and Quickstart were rewritten directly.

Evidence: Repository reference scan and `npm run docs:lint`.

- `patch_quickstart.sh`
- `patch_readme.sh`

## Historical refactor working documents

Deleted because the real Git history preserves the old snapshot and benchmark
record. Enduring dependency rules moved to `docs/architecture.md`; the active
control map moved to `docs/security-test-matrix.md`; current measurements live
in `docs/hardening/baseline.md`, `docs/hardening/results.md`, and generated
reports.

Evidence: `npm run docs:lint`, the documentation index, and repository link
checking.

- `docs/refactor/00-baseline.md`
- `docs/refactor/01-target-architecture.md`
- `docs/refactor/02-security-test-matrix.md`
- `docs/refactor/03-performance-baseline.md`
- `docs/refactor/04-performance-results.md`

No deletion removes a user-managed optional-tool installation, global Codex
configuration, profile credential, session history, or runtime affinity state.
