use super::{
    AppPaths, LastModelSelection, catalog_supports_selection, model_preference_model_is_compatible,
    model_preference_scope, record_model_preference, system_time_nanos,
};
use anyhow::{Result, bail};
use std::path::Path;
use std::time::SystemTime;

pub(crate) fn remember_model_preference_for_launch(
    paths: &AppPaths,
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    model: Option<&str>,
    reasoning_effort: Option<&str>,
    source: &str,
) -> Result<()> {
    let Some(model) = model.map(str::trim).filter(|model| !model.is_empty()) else {
        return Ok(());
    };
    let scope = model_preference_scope(codex_home, codex_args)?;
    let selection = LastModelSelection {
        scope,
        model: model.to_string(),
        reasoning_effort: reasoning_effort
            .map(str::trim)
            .filter(|effort| !effort.is_empty())
            .map(ToOwned::to_owned),
        selected_at: system_time_nanos(SystemTime::now()),
        generation: 0,
        source: source.to_string(),
    };
    if !model_preference_model_is_compatible(codex_home, codex_args, &selection)
        || selection.reasoning_effort.as_deref().is_some_and(|effort| {
            !catalog_supports_selection(codex_home, codex_args, &selection, effort)
        })
    {
        bail!("selected model preference is not valid for the current catalog");
    }
    record_model_preference(paths, selection)
}
