use crate::{AppPaths, ChildProcessPlan, codex_effective_config_value};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MODEL_PREFERENCE_SCHEMA_VERSION: u32 = 1;
const MODEL_PREFERENCE_FILE: &str = "model-preferences.json";
const OPENAI_PROVIDER_ID: &str = "openai";
const GOVERNED_OPENAI_TRANSPORT_PROVIDER_ID: &str = "prodex-openai-governed-http";
const MODEL_PREFERENCE_LOCK_WAIT: Duration = Duration::from_millis(250);

#[path = "runtime_model_preferences_lock.rs"]
mod lock;
#[path = "runtime_model_preferences_pending.rs"]
mod pending;
use lock::try_acquire_model_preference_lock;
use pending::{flush_pending_model_preference, save_pending_model_preference};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelPreferenceScope {
    /// A digest keeps arbitrary provider identifiers, URLs, and user input out of the store.
    pub(crate) provider: String,
    pub(crate) catalog: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LastModelSelection {
    pub(crate) scope: ModelPreferenceScope,
    pub(crate) model: String,
    /// `None` means the config key was absent; `Some("none")` is explicit `none`.
    #[serde(default)]
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) selected_at: u128,
    pub(crate) generation: u64,
    pub(crate) source: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ModelPreferenceFile {
    #[serde(default = "current_model_preference_schema_version")]
    schema_version: u32,
    #[serde(default)]
    selections: Vec<LastModelSelection>,
}
fn current_model_preference_schema_version() -> u32 {
    MODEL_PREFERENCE_SCHEMA_VERSION
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigFingerprint {
    modified_at: u128,
    digest: [u8; 32],
}
#[derive(Debug, Clone)]
struct ConfigSnapshot {
    fingerprint: ConfigFingerprint,
    model: Option<String>,
    reasoning_effort: Option<String>,
}
#[derive(Debug, Clone)]
pub(crate) struct ModelPreferenceContext {
    pub(crate) logical_scope: ModelPreferenceScope,
    pub(crate) remembered: Option<LastModelSelection>,
    explicit_model: Option<String>,
    explicit_effort: Option<String>,
}
pub(crate) fn model_preference_scope(
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
) -> Result<ModelPreferenceScope> {
    let (provider, catalog) = model_preference_provider_and_catalog(codex_home, codex_args)?;
    let provider = logical_model_provider_id(&provider);
    let catalog_identity = match catalog {
        Some(path) => catalog_identity(&PathBuf::from(path), &provider),
        None => generated_catalog_identity(&provider)
            .unwrap_or_else(|| digest_bytes(format!("codex-default-v1\0{provider}").as_bytes())),
    };
    Ok(ModelPreferenceScope {
        provider: digest_bytes(provider.as_bytes()),
        catalog: catalog_identity,
    })
}
fn model_preference_provider_and_catalog(
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
) -> Result<(String, Option<String>)> {
    let provider_args = crate::profile_openai_compatible_codex_args(codex_home, codex_args)?;
    let provider = codex_effective_config_value(codex_home, &provider_args, "model_provider")?
        .unwrap_or_else(|| OPENAI_PROVIDER_ID.to_string());
    let catalog = codex_effective_config_value(codex_home, &provider_args, "model_catalog_json")?;
    Ok((provider, catalog))
}
fn logical_model_provider_id(provider: &str) -> String {
    if provider.eq_ignore_ascii_case(OPENAI_PROVIDER_ID)
        || provider.eq_ignore_ascii_case(GOVERNED_OPENAI_TRANSPORT_PROVIDER_ID)
    {
        OPENAI_PROVIDER_ID.to_string()
    } else {
        provider.to_string()
    }
}
fn catalog_identity(path: &Path, provider: &str) -> String {
    let generated_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            matches!(
                *name,
                "prodex-deepseek-model-catalog.json"
                    | "prodex-external-provider-model-catalog.json"
                    | "prodex-gemini-model-catalog.json"
                    | "prodex-local-model-catalog.json"
                    | "prodex-copilot-runtime-model-catalog.json"
                    | "kiro_model_catalog.json"
            )
        });
    if let Some(name) = generated_name {
        return digest_bytes(format!("prodex-generated-catalog-v1\0{provider}\0{name}").as_bytes());
    }
    let bytes = fs::read(path).unwrap_or_else(|_| path.to_string_lossy().as_bytes().to_vec());
    digest_bytes(&bytes)
}

fn generated_catalog_identity(provider: &str) -> Option<String> {
    let name = match provider.to_ascii_lowercase().as_str() {
        "prodex-deepseek" => "prodex-deepseek-model-catalog.json",
        "prodex-gemini" => "prodex-gemini-model-catalog.json",
        "prodex-local" => "prodex-local-model-catalog.json",
        "prodex-anthropic" | "prodex-copilot" | "prodex-kiro" => {
            "prodex-external-provider-model-catalog.json"
        }
        _ => return None,
    };
    Some(digest_bytes(
        format!("prodex-generated-catalog-v1\0{provider}\0{name}").as_bytes(),
    ))
}
/// Resolves one immutable logical scope and its remembered selection for a launch.
pub(crate) fn resolve_fresh_model_preference_context(
    paths: &AppPaths,
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
) -> Result<ModelPreferenceContext> {
    if let Err(_error) = flush_pending_model_preference(paths) {
        crate::print_launch_status(
            "pending model preference synchronization is unavailable; continuing",
        );
    }
    let logical_scope = model_preference_scope(codex_home, codex_args)?;
    let explicit_model = crate::runtime_launch_cli_model(codex_args)
        .or_else(|| crate::codex_cli_config_override_value(codex_args, "model"));
    let explicit_effort =
        crate::codex_cli_config_override_value(codex_args, "model_reasoning_effort");
    if prodex_runtime_launch::codex_resume_session_id(codex_args).is_some() {
        return Ok(ModelPreferenceContext {
            logical_scope,
            remembered: None,
            explicit_model,
            explicit_effort,
        });
    }
    let remembered = match load_latest_model_preference(paths, &logical_scope) {
        Ok(Some(selection)) => Some(selection),
        Ok(None) => {
            migrate_transport_model_preference(paths, codex_home, codex_args, &logical_scope)?
        }
        Err(_error) => {
            crate::print_launch_status(
                "remembered model preference is unavailable; using normal Codex resolution",
            );
            None
        }
    };
    let remembered =
        if remembered.is_none() && explicit_model.is_none() && explicit_effort.is_none() {
            migrate_native_model_preference(paths, codex_home, codex_args, logical_scope.clone())?
        } else {
            remembered
        };
    Ok(ModelPreferenceContext {
        logical_scope,
        remembered,
        explicit_model,
        explicit_effort,
    })
}
/// Applies remembered fields that were not explicitly supplied for a fresh launch.
///
/// This is called before and after provider catalog preparation. The same context is passed to
/// both calls, so runtime transport arguments cannot change the preference bucket.
pub(crate) fn apply_fresh_model_preference_selection(
    codex_home: &Path,
    mut codex_args: Vec<std::ffi::OsString>,
    context: &ModelPreferenceContext,
    include_model: bool,
    include_effort: bool,
) -> Vec<std::ffi::OsString> {
    let Some(remembered) = context.remembered.as_ref() else {
        return codex_args;
    };
    let mut selection = remembered.clone();
    if let Some(model) = context.explicit_model.as_deref() {
        selection.model = model.to_string();
    }
    if let Some(effort) = context.explicit_effort.as_deref() {
        selection.reasoning_effort = Some(effort.to_string());
    }
    if !model_preference_model_is_compatible(codex_home, &codex_args, &selection) {
        if context.explicit_model.is_none() && !include_model {
            remove_model_preference_override(&mut codex_args);
        }
        return codex_args;
    }
    apply_model_preference_selection(
        codex_home,
        std::mem::take(&mut codex_args),
        &selection,
        include_model && context.explicit_model.is_none(),
        include_effort && context.explicit_effort.is_none(),
    )
}

fn remove_model_preference_override(args: &mut Vec<std::ffi::OsString>) {
    let Some(index) = args.windows(2).position(|window| {
        matches!(window[0].to_str(), Some("-c" | "--config"))
            && window[1]
                .to_str()
                .is_some_and(|assignment| assignment.trim_start().starts_with("model="))
    }) else {
        return;
    };
    args.drain(index..=index + 1);
}

fn migrate_transport_model_preference(
    paths: &AppPaths,
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    logical_scope: &ModelPreferenceScope,
) -> Result<Option<LastModelSelection>> {
    if logical_scope.provider != digest_bytes(OPENAI_PROVIDER_ID.as_bytes()) {
        return Ok(None);
    }
    let (provider, catalog) = model_preference_provider_and_catalog(codex_home, codex_args)?;
    if !provider.eq_ignore_ascii_case(OPENAI_PROVIDER_ID) {
        return Ok(None);
    }
    let legacy_provider = digest_bytes(GOVERNED_OPENAI_TRANSPORT_PROVIDER_ID.as_bytes());
    let mut legacy_catalogs = vec![logical_scope.catalog.clone()];
    legacy_catalogs.push(match catalog {
        Some(path) => catalog_identity(&PathBuf::from(path), GOVERNED_OPENAI_TRANSPORT_PROVIDER_ID),
        None => digest_bytes(
            format!("codex-default-v1\0{GOVERNED_OPENAI_TRANSPORT_PROVIDER_ID}").as_bytes(),
        ),
    });
    legacy_catalogs.sort();
    legacy_catalogs.dedup();
    for catalog in legacy_catalogs {
        let legacy_scope = ModelPreferenceScope {
            provider: legacy_provider.clone(),
            catalog,
        };
        let Some(selection) = load_latest_model_preference(paths, &legacy_scope)? else {
            continue;
        };
        if !model_preference_model_is_compatible(codex_home, codex_args, &selection)
            || selection.reasoning_effort.as_deref().is_some_and(|effort| {
                !catalog_supports_selection(codex_home, codex_args, &selection, effort)
            })
        {
            continue;
        }
        let migrated = LastModelSelection {
            scope: logical_scope.clone(),
            ..selection
        };
        if let Err(_error) = record_model_preference(paths, migrated.clone()) {
            crate::print_launch_status(
                "legacy model preference migration was unavailable; continuing",
            );
        }
        return Ok(Some(migrated));
    }
    Ok(None)
}

fn migrate_native_model_preference(
    paths: &AppPaths,
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    scope: ModelPreferenceScope,
) -> Result<Option<LastModelSelection>> {
    let Some(snapshot) = read_config_snapshot(&config_paths_for_child(codex_home, codex_args))?
    else {
        return Ok(None);
    };
    let Some(model) = snapshot.model.clone() else {
        return Ok(None);
    };
    let selection = LastModelSelection {
        scope,
        model,
        reasoning_effort: snapshot.reasoning_effort,
        selected_at: snapshot.fingerprint.modified_at,
        generation: 0,
        source: "codex-config-migration".to_string(),
    };
    if record_model_preference(paths, selection.clone()).is_err() {
        crate::print_launch_status(
            "native Codex model preference migration was unavailable; continuing",
        );
    }
    Ok(Some(selection))
}

pub(crate) fn apply_model_preference_selection(
    codex_home: &Path,
    mut codex_args: Vec<std::ffi::OsString>,
    selection: &LastModelSelection,
    include_model: bool,
    include_effort: bool,
) -> Vec<std::ffi::OsString> {
    if include_model {
        prepend_config_override(&mut codex_args, "model", &selection.model);
    }
    if include_effort
        && let Some(effort) = selection.reasoning_effort.as_deref()
        && catalog_supports_selection(codex_home, &codex_args, selection, effort)
    {
        prepend_config_override(&mut codex_args, "model_reasoning_effort", effort);
    }
    codex_args
}

pub(crate) fn model_preference_model_is_compatible(
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    selection: &LastModelSelection,
) -> bool {
    match catalog_model(codex_home, codex_args, selection) {
        None => true,
        Some(model) => model.is_some(),
    }
}

#[cfg(test)]
fn apply_fresh_model_preference(
    paths: &AppPaths,
    codex_home: &Path,
    codex_args: Vec<std::ffi::OsString>,
    include_model: bool,
    include_effort: bool,
) -> Result<Vec<std::ffi::OsString>> {
    let context = resolve_fresh_model_preference_context(paths, codex_home, &codex_args)?;
    Ok(apply_fresh_model_preference_selection(
        codex_home,
        codex_args,
        &context,
        include_model,
        include_effort,
    ))
}

fn catalog_supports_selection(
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    selection: &LastModelSelection,
    effort: &str,
) -> bool {
    let Some(model) = catalog_model(codex_home, codex_args, selection) else {
        return true;
    };
    let Some(model) = model else {
        return false;
    };
    let Some(levels) = model
        .get("supported_reasoning_levels")
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    levels.iter().any(|level| {
        level
            .get("effort")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == effort)
    })
}

fn catalog_model(
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    selection: &LastModelSelection,
) -> Option<Option<serde_json::Value>> {
    let path = crate::codex_effective_config_value(codex_home, codex_args, "model_catalog_json")
        .ok()
        .flatten()?;
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return Some(None),
    };
    let catalog = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(catalog) => catalog,
        Err(_) => return Some(None),
    };
    let Some(models) = catalog.get("models").and_then(serde_json::Value::as_array) else {
        return Some(None);
    };
    Some(
        models
            .iter()
            .find(|model| {
                model
                    .get("slug")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|slug| slug == selection.model)
            })
            .cloned(),
    )
}

fn prepend_config_override(args: &mut Vec<std::ffi::OsString>, key: &str, value: &str) {
    args.splice(
        0..0,
        [
            std::ffi::OsString::from("-c"),
            std::ffi::OsString::from(format!(
                "{key}={}",
                crate::runtime_catalog_config::toml_string_literal(value)
            )),
        ],
    );
}
fn model_preference_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(MODEL_PREFERENCE_FILE)
}

fn model_preference_backup_path(path: &Path) -> PathBuf {
    crate::last_good_file_path(path)
}

fn load_latest_model_preference(
    paths: &AppPaths,
    scope: &ModelPreferenceScope,
) -> Result<Option<LastModelSelection>> {
    let file = model_preference_file_path(paths);
    let backup = model_preference_backup_path(&file);
    let Some((preferences, _generation)) = read_model_preference_file(&file, &backup)? else {
        return Ok(None);
    };
    Ok(preferences
        .selections
        .into_iter()
        .filter(|selection| selection.scope == *scope)
        .max_by_key(|selection| (selection.selected_at, selection.generation)))
}

fn read_model_preference_file(
    path: &Path,
    backup_path: &Path,
) -> Result<Option<(ModelPreferenceFile, u64)>> {
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    let loaded = crate::runtime_store::read_versioned_json_file_with_backup::<ModelPreferenceFile>(
        path,
        backup_path,
    )?;
    validate_model_preference_file(loaded.value, loaded.generation).map(Some)
}

fn read_model_preference_file_locked(
    path: &Path,
    backup_path: &Path,
) -> Result<Option<(ModelPreferenceFile, u64)>> {
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    let primary = fs::read_to_string(path)
        .ok()
        .and_then(|content| crate::runtime_store::parse_versioned_json_or_raw(&content).ok());
    if let Some((preferences, generation)) = primary {
        return validate_model_preference_file(preferences, generation).map(Some);
    }
    let backup = fs::read_to_string(backup_path)
        .with_context(|| format!("failed to read {}", backup_path.display()))?;
    let (preferences, generation) = crate::runtime_store::parse_versioned_json_or_raw(&backup)
        .with_context(|| format!("failed to parse {}", backup_path.display()))?;
    let validated = validate_model_preference_file(preferences, generation)?;
    let _ = crate::runtime_store::write_private_file_atomic(path, backup.as_bytes());
    Ok(Some(validated))
}

fn validate_model_preference_file(
    preferences: ModelPreferenceFile,
    generation: u64,
) -> Result<(ModelPreferenceFile, u64)> {
    if preferences.schema_version > MODEL_PREFERENCE_SCHEMA_VERSION {
        bail!(
            "unsupported model preference schema version {}",
            preferences.schema_version
        );
    }
    Ok((preferences, generation))
}

fn record_model_preference(paths: &AppPaths, mut selection: LastModelSelection) -> Result<()> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let path = model_preference_file_path(paths);
    let backup = model_preference_backup_path(&path);
    let Some(_lock) =
        try_acquire_model_preference_lock(&path, Instant::now() + MODEL_PREFERENCE_LOCK_WAIT)?
    else {
        bail!("model preference file is busy");
    };
    let (mut preferences, generation) = read_model_preference_file_locked(&path, &backup)?
        .unwrap_or((
            ModelPreferenceFile {
                schema_version: MODEL_PREFERENCE_SCHEMA_VERSION,
                selections: Vec::new(),
            },
            0,
        ));
    preferences.schema_version = MODEL_PREFERENCE_SCHEMA_VERSION;
    if let Some(existing) = preferences
        .selections
        .iter()
        .find(|existing| existing.scope == selection.scope)
        && (existing.selected_at, existing.generation) > (selection.selected_at, generation)
    {
        return Ok(());
    }
    let next_generation = generation.saturating_add(1);
    selection.generation = next_generation;
    preferences
        .selections
        .retain(|existing| existing.scope != selection.scope);
    preferences.selections.push(selection);
    crate::runtime_store::write_versioned_json_file_with_backup(
        &path,
        &backup,
        next_generation,
        &preferences,
    )
}

pub(crate) struct ModelPreferenceSync {
    paths: AppPaths,
    config_paths: Vec<PathBuf>,
    scope: ModelPreferenceScope,
    previous: Option<ConfigSnapshot>,
    finished: bool,
}

impl ModelPreferenceSync {
    pub(crate) fn start_with_scope(
        paths: &AppPaths,
        child: &ChildProcessPlan,
        scope: ModelPreferenceScope,
    ) -> Result<Self> {
        let started = Instant::now();
        let config_paths = config_paths_for_child(&child.codex_home, &child.args);
        let baseline = read_config_snapshot(&config_paths)?;
        emit_model_preference_timing("startup.model_preference_resolve_ms", started);
        Ok(Self {
            paths: paths.clone(),
            config_paths,
            scope,
            previous: baseline,
            finished: false,
        })
    }

    pub(crate) fn finish(&mut self) -> Option<String> {
        if self.finished {
            return None;
        }
        self.finished = true;
        let started = Instant::now();
        let result = capture_changed_config(
            &self.paths,
            &self.config_paths,
            &self.scope,
            &mut self.previous,
        )
        .err()
        .map(|error| redacted_preference_error(&error));
        emit_model_preference_timing("shutdown.model_preference_sync_ms", started);
        result
    }
}

impl Drop for ModelPreferenceSync {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn capture_changed_config(
    paths: &AppPaths,
    config_paths: &[PathBuf],
    scope: &ModelPreferenceScope,
    previous: &mut Option<ConfigSnapshot>,
) -> Result<()> {
    let Some(snapshot) = read_config_snapshot(config_paths)? else {
        return Ok(());
    };
    if previous
        .as_ref()
        .is_some_and(|previous| previous.fingerprint == snapshot.fingerprint)
    {
        return Ok(());
    }
    let selection_changed = previous.as_ref().is_none_or(|previous| {
        previous.model != snapshot.model || previous.reasoning_effort != snapshot.reasoning_effort
    });
    if !selection_changed {
        *previous = Some(snapshot);
        return Ok(());
    }
    let Some(model) = snapshot.model.clone() else {
        *previous = Some(snapshot);
        return Ok(());
    };
    let selection = LastModelSelection {
        scope: scope.clone(),
        model,
        reasoning_effort: snapshot.reasoning_effort.clone(),
        selected_at: snapshot.fingerprint.modified_at,
        generation: 0,
        source: "codex-config".to_string(),
    };
    let result = record_model_preference(paths, selection.clone());
    if result.is_ok() {
        *previous = Some(snapshot);
    } else if let Err(_error) = save_pending_model_preference(paths, &selection) {
        // The original error remains the observable result; a later launch can retry the write.
    }
    result
}

fn read_config_snapshot(paths: &[PathBuf]) -> Result<Option<ConfigSnapshot>> {
    let mut model = None;
    let mut reasoning_effort = None;
    let mut modified_at = 0;
    let mut digest = Sha256::new();
    let mut found = false;
    for path in paths {
        let raw = match fs::read(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        };
        found = true;
        digest.update(path.file_name().unwrap_or_default().as_encoded_bytes());
        digest.update(&raw);
        let value: toml::Value = toml::from_slice(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let table = value
            .as_table()
            .context("Codex config root must be a TOML table")?;
        if let Some(value) = table.get("model") {
            model = value
                .as_str()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .map(str::to_string);
        }
        if let Some(value) = table.get("model_reasoning_effort") {
            reasoning_effort = Some(
                value
                    .as_str()
                    .map(str::to_string)
                    .context("model_reasoning_effort must be a TOML string")?,
            );
        }
        modified_at = modified_at.max(
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .map(system_time_nanos)
                .unwrap_or_default(),
        );
    }
    if !found {
        return Ok(None);
    }
    let digest: [u8; 32] = digest.finalize().into();
    Ok(Some(ConfigSnapshot {
        fingerprint: ConfigFingerprint {
            modified_at,
            digest,
        },
        model,
        reasoning_effort,
    }))
}

fn config_paths_for_child(codex_home: &Path, codex_args: &[std::ffi::OsString]) -> Vec<PathBuf> {
    let mut paths = vec![codex_home.join("config.toml")];
    if let Some(profile) = crate::codex_cli_profile_v2_name(codex_args)
        && let Some(path) = crate::codex_profile_v2_config_path(codex_home, &profile)
    {
        paths.push(path);
    }
    paths
}

fn redacted_preference_error(_error: &anyhow::Error) -> String {
    "model preference synchronization failed".to_string()
}

fn emit_model_preference_timing(stage: &str, started: Instant) {
    if std::env::var_os("PRODEX_RUNTIME_TIMINGS").is_some() {
        eprintln!(
            "prodex_runtime_timing stage={stage} duration_ms={}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
fn now_nanos() -> u128 {
    system_time_nanos(SystemTime::now())
}

fn system_time_nanos(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
#[path = "runtime_model_preferences_tests.rs"]
mod extracted_tests;
#[cfg(test)]
#[path = "runtime_model_preferences_regression_tests.rs"]
mod regression_tests;
