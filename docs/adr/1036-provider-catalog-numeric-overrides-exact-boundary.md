# ADR 1036: Provider catalog numeric overrides exact boundary

## Status

Accepted.

## Context

Local, DeepSeek, Gemini, Anthropic, and Copilot runtime catalog preparation accepts
`model_context_window` and `model_auto_compact_token_limit` overrides from CLI
`-c` values or Codex config. These values drive model catalog capability
metadata used by local rewriting and model negotiation. Previously the catalog
helpers trimmed malformed values, ignored parse failures, and silently fell back
to provider defaults.

Affected symbols:

- `runtime_deepseek_config::deepseek_u64_config_for_launch`
- `runtime_gemini_config::gemini_u64_config_for_launch`
- `runtime_external_provider_config::external_catalog_u64_config_for_launch`
- `runtime_local_provider_config::local_u64_config_for_launch`
- `codex_config::codex_cli_config_override_exact_value`
- `codex_config::codex_config_exact_value`

The risk is silent capability drift: a typo in a launch/config override can
publish an unintended context window or compaction limit and hide the operator
mistake behind defaults.

## Decision

Provider catalog numeric overrides are exact positive integer values greater
than one. Empty, whitespace-bearing, non-numeric, zero, or one-valued overrides
now fail closed during catalog preparation instead of falling back to defaults.

## Consequences

Operator mistakes in provider capability metadata are visible during launch.
Unset values continue to use provider defaults.

Regression coverage:

- `deepseek_provider_codex_args_rejects_invalid_numeric_catalog_overrides`
- `gemini_provider_codex_args_rejects_invalid_numeric_catalog_overrides`
- `external_provider_catalog_args_rejects_invalid_numeric_overrides`
- `local_provider_catalog_args_rejects_invalid_numeric_overrides`
- `exact_cli_override_preserves_empty_and_quoted_whitespace`
- `exact_config_value_preserves_empty_and_whitespace_strings`
