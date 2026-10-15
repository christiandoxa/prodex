# OpenAI catalog retirement filter migration

The OpenAI model picker now delegates retired-model filtering to
`prodex_mojo_rich_catalog_openai_retired_v1` in
`mojo/prodex_core/rich_catalog.mojo`. The Mojo predicate preserves Rust's
Unicode trim, ASCII case folding, and exact `spark`, `gpt-5.3-codex-spark`, and
`gpt-5.3-spark` matches. Rust keeps cache/configuration I/O and choice
materialization, and compiled ABI plus picker regressions cover the boundary.
