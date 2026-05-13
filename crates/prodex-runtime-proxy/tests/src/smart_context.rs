use super::*;
use std::borrow::Cow;

#[path = "smart_context/golden.rs"]
mod golden;

#[path = "smart_context/core_artifacts.rs"]
mod core_artifacts;

#[path = "smart_context/memory_budget.rs"]
mod memory_budget;

#[path = "smart_context/rehydration.rs"]
mod rehydration;

#[path = "smart_context/token_accounting.rs"]
mod token_accounting;

#[path = "smart_context/static_context.rs"]
mod static_context;

#[path = "smart_context/adaptive_rewrite.rs"]
mod adaptive_rewrite;

#[path = "smart_context/safety.rs"]
mod safety;

#[path = "smart_context/tool_outputs.rs"]
mod tool_outputs;
