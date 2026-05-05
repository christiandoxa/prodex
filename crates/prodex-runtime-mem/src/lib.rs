use anyhow::{Context, Result, bail};
use dirs::home_dir;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

const DEFAULT_CLAUDE_CONFIG_DIR_NAME: &str = ".claude";
const CLAUDE_MEM_DATA_DIR_NAME: &str = ".claude-mem";
const CLAUDE_MEM_SETTINGS_FILE_NAME: &str = "settings.json";
const CLAUDE_MEM_TRANSCRIPT_WATCH_FILE_NAME: &str = "transcript-watch.json";
const CLAUDE_MEM_TRANSCRIPT_WATCH_STATE_FILE_NAME: &str = "transcript-watch-state.json";
const CLAUDE_MEM_PLUGIN_MARKETPLACE_OWNER: &str = "thedotmack";
const CLAUDE_MEM_CODEX_SCHEMA_NAME: &str = "codex";
const CLAUDE_MEM_PRODEX_WATCH_NAME_PREFIX: &str = "prodex-codex-";
const CLAUDE_MEM_CLAUDE_CODE_PATH_SETTING: &str = "CLAUDE_CODE_PATH";
const PRODEX_CLAUDE_MEM_DIR_NAME: &str = "claude-mem";
const PRODEX_CLAUDE_MEM_WRAPPER_NAME: &str = "prodex-claude";
const CLAUDE_MEM_PREFIX: &str = "mem";
const CLAUDE_MEM_FULL_PREFIX: &str = "mem-full";
const CLAUDE_MEM_SUPER_SLIM_PREFIX: &str = "mem-super-slim";
const CLAUDE_MEM_FULL_FLAG: &str = "--mem-full";
const CLAUDE_MEM_SUPER_SLIM_FLAG: &str = "--mem-super-slim";
const RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS: &[&str] =
    &["payload.prompt_summary", "payload.metadata.prompt_summary"];
const RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS: &[&str] = &[
    "payload.metadata.artifact_ref",
    "payload.metadata.artifact_id",
    "payload.metadata.artifactId",
    "payload.artifact.reference",
    "payload.artifact.ref",
    "payload.artifact.id",
    "payload.artifact_id",
    "payload.artifactId",
];
const RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS: &[&str] =
    &["payload.summary", "payload.metadata.summary"];
const RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS: &[&str] = &[
    "payload.metadata.artifact_ref",
    "payload.metadata.artifact_id",
    "payload.metadata.artifactId",
    "payload.artifact.reference",
    "payload.artifact.ref",
    "payload.artifact.id",
    "payload.artifact_id",
    "payload.artifactId",
];
const RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS: &[&str] = &["payload.summary"];
const RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT: usize = 180;
const RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT: usize = 72;
pub const RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const RUNTIME_MEM_DEFAULT_CAPSULE_MINIMAL_TOKEN_BUDGET: usize = 128;
pub const RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET: usize = 512;
pub const RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET: usize = 2_048;
pub const RUNTIME_MEM_SUPER_CAPSULE_MINIMAL_TOKEN_BUDGET: usize = 256;
pub const RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET: usize = 1_024;
pub const RUNTIME_MEM_SUPER_CAPSULE_LARGE_TOKEN_BUDGET: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemTranscriptMode {
    Slim,
    SuperSlim,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemSchemaSelectionPolicy {
    Explicit(RuntimeMemTranscriptMode),
    SafeSuperSlimCandidate {
        fallback_mode: RuntimeMemTranscriptMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsulePriority {
    Required,
    ProjectLocal,
    Recent,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsuleBudgetTier {
    Exact,
    Large,
    Condensed,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsuleBudgetMode {
    Default,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsuleBudget {
    Explicit(usize),
    Tier {
        available_tokens: usize,
        mode: RuntimeMemCapsuleBudgetMode,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMemCapsuleMetadata {
    pub id: String,
    pub token_cost: usize,
    pub required: bool,
    pub project_path: Option<PathBuf>,
    pub updated_at_seconds: Option<i64>,
    pub relevance: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeMemRecallIntent {
    pub paths: Vec<PathBuf>,
    pub symbols: Vec<String>,
}

impl RuntimeMemRecallIntent {
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.symbols.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMemRecallCapsuleMetadata {
    pub capsule: RuntimeMemCapsuleMetadata,
    pub paths: Vec<PathBuf>,
    pub symbols: Vec<String>,
}

impl RuntimeMemRecallCapsuleMetadata {
    pub fn new(capsule: RuntimeMemCapsuleMetadata) -> Self {
        Self {
            capsule,
            paths: Vec::new(),
            symbols: Vec::new(),
        }
    }
}

impl From<RuntimeMemCapsuleMetadata> for RuntimeMemRecallCapsuleMetadata {
    fn from(capsule: RuntimeMemCapsuleMetadata) -> Self {
        Self::new(capsule)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemRecallDedupeItem {
    pub id: String,
    pub content: String,
    pub required: bool,
    pub artifact_ref: Option<String>,
}

impl RuntimeMemRecallDedupeItem {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            required: false,
            artifact_ref: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeMemRecallDedupeReason {
    Duplicate { original_id: String },
    ArtifactRef { artifact_ref: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemRecallDedupeEntry {
    pub id: String,
    pub content: String,
    pub content_hash: String,
    pub required: bool,
    pub replacement: Option<String>,
    pub reason: Option<RuntimeMemRecallDedupeReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemCapsuleSelectionContext {
    pub token_budget: usize,
    pub project_root: Option<PathBuf>,
    pub now_seconds: Option<i64>,
    pub recent_window_seconds: u64,
}

impl RuntimeMemCapsuleSelectionContext {
    pub fn new(token_budget: usize) -> Self {
        Self {
            token_budget,
            project_root: None,
            now_seconds: None,
            recent_window_seconds: RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemAutoCapsuleSelectionContext {
    pub budget: RuntimeMemCapsuleBudget,
    pub project_root: Option<PathBuf>,
    pub now_seconds: Option<i64>,
    pub recent_window_seconds: u64,
}

impl RuntimeMemAutoCapsuleSelectionContext {
    pub fn new(budget: RuntimeMemCapsuleBudget) -> Self {
        Self {
            budget,
            project_root: None,
            now_seconds: None,
            recent_window_seconds: RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS,
        }
    }

    pub fn super_mode(available_tokens: usize) -> Self {
        Self::new(RuntimeMemCapsuleBudget::Tier {
            available_tokens,
            mode: RuntimeMemCapsuleBudgetMode::Super,
        })
    }

    pub fn to_selection_context(&self) -> RuntimeMemCapsuleSelectionContext {
        RuntimeMemCapsuleSelectionContext {
            token_budget: runtime_mem_capsule_token_budget(self.budget),
            project_root: self.project_root.clone(),
            now_seconds: self.now_seconds,
            recent_window_seconds: self.recent_window_seconds,
        }
    }
}

impl Default for RuntimeMemCapsuleSelectionContext {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemCapsuleSelectionEntry {
    pub id: String,
    pub priority: RuntimeMemCapsulePriority,
    pub token_cost: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemCapsuleSelection {
    pub selected: Vec<RuntimeMemCapsuleSelectionEntry>,
    pub omitted: Vec<RuntimeMemCapsuleSelectionEntry>,
    pub used_tokens: usize,
    pub token_budget: usize,
}

pub fn runtime_mem_classify_capsule(
    capsule: &RuntimeMemCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
) -> RuntimeMemCapsulePriority {
    if capsule.required {
        return RuntimeMemCapsulePriority::Required;
    }
    if runtime_mem_capsule_is_project_local(capsule, context) {
        return RuntimeMemCapsulePriority::ProjectLocal;
    }
    if runtime_mem_capsule_is_recent(capsule, context) {
        return RuntimeMemCapsulePriority::Recent;
    }
    RuntimeMemCapsulePriority::Optional
}

pub fn runtime_mem_capsule_budget_tier(available_tokens: usize) -> RuntimeMemCapsuleBudgetTier {
    match available_tokens {
        16_000.. => RuntimeMemCapsuleBudgetTier::Exact,
        8_000..=15_999 => RuntimeMemCapsuleBudgetTier::Large,
        2_000..=7_999 => RuntimeMemCapsuleBudgetTier::Condensed,
        _ => RuntimeMemCapsuleBudgetTier::Minimal,
    }
}

pub fn runtime_mem_capsule_token_budget(budget: RuntimeMemCapsuleBudget) -> usize {
    match budget {
        RuntimeMemCapsuleBudget::Explicit(token_budget) => token_budget,
        RuntimeMemCapsuleBudget::Tier {
            available_tokens,
            mode,
        } => runtime_mem_capsule_token_budget_for_tier(
            mode,
            runtime_mem_capsule_budget_tier(available_tokens),
        ),
    }
}

pub fn runtime_mem_capsule_token_budget_for_tier(
    mode: RuntimeMemCapsuleBudgetMode,
    tier: RuntimeMemCapsuleBudgetTier,
) -> usize {
    match mode {
        RuntimeMemCapsuleBudgetMode::Default => match tier {
            RuntimeMemCapsuleBudgetTier::Minimal => {
                RUNTIME_MEM_DEFAULT_CAPSULE_MINIMAL_TOKEN_BUDGET
            }
            RuntimeMemCapsuleBudgetTier::Condensed => {
                RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            RuntimeMemCapsuleBudgetTier::Large | RuntimeMemCapsuleBudgetTier::Exact => {
                RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET
            }
        },
        RuntimeMemCapsuleBudgetMode::Super => match tier {
            RuntimeMemCapsuleBudgetTier::Minimal => RUNTIME_MEM_SUPER_CAPSULE_MINIMAL_TOKEN_BUDGET,
            RuntimeMemCapsuleBudgetTier::Condensed => {
                RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            RuntimeMemCapsuleBudgetTier::Large | RuntimeMemCapsuleBudgetTier::Exact => {
                RUNTIME_MEM_SUPER_CAPSULE_LARGE_TOKEN_BUDGET
            }
        },
    }
}

pub fn runtime_mem_select_capsules_auto(
    capsules: impl IntoIterator<Item = RuntimeMemCapsuleMetadata>,
    context: RuntimeMemAutoCapsuleSelectionContext,
) -> RuntimeMemCapsuleSelection {
    runtime_mem_select_capsules(capsules, context.to_selection_context())
}

pub fn runtime_mem_select_capsules_for_recall_diet(
    capsules: impl IntoIterator<Item = RuntimeMemRecallCapsuleMetadata>,
    context: RuntimeMemAutoCapsuleSelectionContext,
    intent: RuntimeMemRecallIntent,
) -> RuntimeMemCapsuleSelection {
    runtime_mem_select_capsules_with_recall_intent(capsules, context.to_selection_context(), intent)
}

pub fn runtime_mem_select_capsules_with_recall_intent(
    capsules: impl IntoIterator<Item = RuntimeMemRecallCapsuleMetadata>,
    context: RuntimeMemCapsuleSelectionContext,
    intent: RuntimeMemRecallIntent,
) -> RuntimeMemCapsuleSelection {
    if intent.is_empty() {
        return runtime_mem_select_capsules(
            capsules.into_iter().map(|candidate| candidate.capsule),
            context,
        );
    }

    let mut candidates = capsules
        .into_iter()
        .map(|capsule| {
            let priority = runtime_mem_classify_capsule(&capsule.capsule, &context);
            let intent_score = runtime_mem_capsule_intent_score(&capsule, &context, &intent);
            (capsule, priority, intent_score)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(runtime_mem_recall_diet_capsule_order);

    let mut selected = Vec::new();
    let mut omitted = Vec::new();
    let mut used_tokens = 0usize;

    for (candidate, priority, _) in candidates {
        let entry = RuntimeMemCapsuleSelectionEntry {
            id: candidate.capsule.id,
            priority,
            token_cost: candidate.capsule.token_cost,
        };
        if used_tokens.saturating_add(entry.token_cost) <= context.token_budget {
            used_tokens += entry.token_cost;
            selected.push(entry);
        } else {
            omitted.push(entry);
        }
    }

    RuntimeMemCapsuleSelection {
        selected,
        omitted,
        used_tokens,
        token_budget: context.token_budget,
    }
}

pub fn runtime_mem_select_capsules(
    capsules: impl IntoIterator<Item = RuntimeMemCapsuleMetadata>,
    context: RuntimeMemCapsuleSelectionContext,
) -> RuntimeMemCapsuleSelection {
    let mut candidates = capsules
        .into_iter()
        .map(|capsule| {
            let priority = runtime_mem_classify_capsule(&capsule, &context);
            (capsule, priority)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(runtime_mem_capsule_order);

    let mut selected = Vec::new();
    let mut omitted = Vec::new();
    let mut used_tokens = 0usize;

    for (capsule, priority) in candidates {
        let entry = RuntimeMemCapsuleSelectionEntry {
            id: capsule.id,
            priority,
            token_cost: capsule.token_cost,
        };
        if used_tokens.saturating_add(entry.token_cost) <= context.token_budget {
            used_tokens += entry.token_cost;
            selected.push(entry);
        } else {
            omitted.push(entry);
        }
    }

    RuntimeMemCapsuleSelection {
        selected,
        omitted,
        used_tokens,
        token_budget: context.token_budget,
    }
}

pub fn runtime_mem_dedupe_recall_content(
    items: impl IntoIterator<Item = RuntimeMemRecallDedupeItem>,
) -> Vec<RuntimeMemRecallDedupeEntry> {
    let items = items.into_iter().collect::<Vec<_>>();
    let mut first_by_content = HashMap::<String, String>::new();
    let mut first_required_by_content = HashMap::<String, String>::new();

    for item in &items {
        first_by_content
            .entry(item.content.clone())
            .or_insert_with(|| item.id.clone());
        if item.required {
            first_required_by_content
                .entry(item.content.clone())
                .or_insert_with(|| item.id.clone());
        }
    }

    items
        .into_iter()
        .map(|item| {
            let content_hash = runtime_mem_content_hash(&item.content);
            if item.required {
                return RuntimeMemRecallDedupeEntry {
                    id: item.id,
                    content: item.content,
                    content_hash,
                    required: true,
                    replacement: None,
                    reason: None,
                };
            }

            let artifact_ref =
                runtime_mem_prodex_artifact_ref(item.artifact_ref.as_deref(), &item.content);
            if let Some(artifact_ref) = artifact_ref {
                let original_bytes = item.content.len();
                return RuntimeMemRecallDedupeEntry {
                    id: item.id,
                    content: item.content,
                    replacement: Some(runtime_mem_artifact_recall_summary(
                        &artifact_ref,
                        &content_hash,
                        original_bytes,
                    )),
                    reason: Some(RuntimeMemRecallDedupeReason::ArtifactRef { artifact_ref }),
                    content_hash,
                    required: false,
                };
            }

            let original_id = first_required_by_content
                .get(&item.content)
                .or_else(|| first_by_content.get(&item.content))
                .cloned();
            if let Some(original_id) = original_id
                && original_id != item.id
            {
                let original_bytes = item.content.len();
                return RuntimeMemRecallDedupeEntry {
                    id: item.id,
                    content: item.content,
                    replacement: Some(runtime_mem_duplicate_recall_summary(
                        &original_id,
                        &content_hash,
                        original_bytes,
                    )),
                    reason: Some(RuntimeMemRecallDedupeReason::Duplicate { original_id }),
                    content_hash,
                    required: false,
                };
            }

            RuntimeMemRecallDedupeEntry {
                id: item.id,
                content: item.content,
                content_hash,
                required: false,
                replacement: None,
                reason: None,
            }
        })
        .collect()
}

pub fn runtime_mem_content_hash(text: &str) -> String {
    format!("sc:{:016x}", runtime_mem_fnv1a64(text.as_bytes()))
}

pub fn runtime_mem_extract_mode(args: &[OsString]) -> (bool, Vec<OsString>) {
    let (mode, args) = runtime_mem_extract_mode_with_detail(args);
    (mode.is_some(), args)
}

pub fn runtime_mem_super_default_transcript_mode(
    mode: Option<RuntimeMemTranscriptMode>,
) -> Option<RuntimeMemTranscriptMode> {
    match mode {
        Some(RuntimeMemTranscriptMode::Slim) => Some(RuntimeMemTranscriptMode::SuperSlim),
        other => other,
    }
}

pub fn runtime_mem_select_codex_schema_mode_for_event(
    policy: RuntimeMemSchemaSelectionPolicy,
    event: &Value,
) -> RuntimeMemTranscriptMode {
    match policy {
        RuntimeMemSchemaSelectionPolicy::Explicit(mode) => mode,
        RuntimeMemSchemaSelectionPolicy::SafeSuperSlimCandidate { fallback_mode } => {
            runtime_mem_safe_auto_codex_schema_mode_for_event(fallback_mode, event)
        }
    }
}

pub fn runtime_mem_safe_auto_codex_schema_mode_for_event(
    fallback_mode: RuntimeMemTranscriptMode,
    event: &Value,
) -> RuntimeMemTranscriptMode {
    if matches!(fallback_mode, RuntimeMemTranscriptMode::Full) {
        return RuntimeMemTranscriptMode::Full;
    }
    if runtime_mem_event_has_super_slim_prompt_reference(event) {
        RuntimeMemTranscriptMode::SuperSlim
    } else {
        RuntimeMemTranscriptMode::Slim
    }
}

pub fn runtime_mem_codex_schema_for_safe_auto_event(
    fallback_mode: RuntimeMemTranscriptMode,
    event: &Value,
) -> Value {
    runtime_mem_codex_schema_for_mode(runtime_mem_safe_auto_codex_schema_mode_for_event(
        fallback_mode,
        event,
    ))
}

pub fn runtime_mem_event_has_super_slim_prompt_reference(event: &Value) -> bool {
    RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS
        .iter()
        .any(|path| {
            runtime_mem_lookup_json_path(event, path).is_some_and(runtime_mem_value_is_text)
        })
        || RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS
            .iter()
            .any(|path| {
                runtime_mem_lookup_json_path(event, path).is_some_and(runtime_mem_value_is_text)
            })
        || runtime_mem_value_contains_artifact_marker(event)
}

pub fn runtime_mem_super_slim_shadow_codex_event(event: &Value) -> Value {
    let mut shadow = event.clone();
    let Some(payload_type) = runtime_mem_lookup_json_path(&shadow, "payload.type")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return shadow;
    };

    match payload_type.as_str() {
        "user_message" => runtime_mem_shadow_user_message(&mut shadow),
        "agent_message" => runtime_mem_shadow_assistant_message(&mut shadow),
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
            runtime_mem_shadow_tool_output(&mut shadow)
        }
        _ => {}
    }
    shadow
}

pub fn runtime_mem_super_slim_shadow_codex_events<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut dedupe_state = RuntimeMemEventDedupeState::default();
    events
        .into_iter()
        .enumerate()
        .map(|(index, event)| {
            runtime_mem_super_slim_shadow_codex_event_with_dedupe(event, index, &mut dedupe_state)
        })
        .collect()
}

pub fn runtime_mem_extract_mode_with_detail(
    args: &[OsString],
) -> (Option<RuntimeMemTranscriptMode>, Vec<OsString>) {
    let Some(first) = args.first().and_then(|arg| arg.to_str()) else {
        return (None, args.to_vec());
    };
    if first == CLAUDE_MEM_SUPER_SLIM_PREFIX {
        return (
            Some(RuntimeMemTranscriptMode::SuperSlim),
            args[1..].to_vec(),
        );
    }
    if first == CLAUDE_MEM_FULL_PREFIX {
        return (Some(RuntimeMemTranscriptMode::Full), args[1..].to_vec());
    }
    if first != CLAUDE_MEM_PREFIX {
        return (None, args.to_vec());
    }
    if args
        .get(1)
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == CLAUDE_MEM_SUPER_SLIM_FLAG)
    {
        return (
            Some(RuntimeMemTranscriptMode::SuperSlim),
            args[2..].to_vec(),
        );
    }
    if args
        .get(1)
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == CLAUDE_MEM_FULL_FLAG)
    {
        return (Some(RuntimeMemTranscriptMode::Full), args[2..].to_vec());
    }
    (Some(RuntimeMemTranscriptMode::Slim), args[1..].to_vec())
}

pub fn runtime_mem_claude_plugin_dir() -> Result<PathBuf> {
    let home = home_dir().context("failed to determine home directory for claude-mem")?;
    let plugin_dir = runtime_mem_claude_plugin_dir_from_home(&home);
    let manifest_path = runtime_mem_claude_plugin_manifest_path(&plugin_dir);
    if !manifest_path.is_file() {
        bail!(
            "claude-mem is not installed for Claude Code; run `npx claude-mem install --ide claude-code` first"
        );
    }
    Ok(plugin_dir)
}

pub fn ensure_runtime_mem_codex_watch_for_home(codex_home: &Path) -> Result<()> {
    ensure_runtime_mem_codex_watch_for_home_with_mode(codex_home, RuntimeMemTranscriptMode::Slim)
}

pub fn ensure_runtime_mem_codex_watch_for_home_with_mode(
    codex_home: &Path,
    mode: RuntimeMemTranscriptMode,
) -> Result<()> {
    let home = home_dir().context("failed to determine home directory for claude-mem")?;
    let config_path = runtime_mem_transcript_watch_config_path_from_home(&home);
    ensure_runtime_mem_codex_watch_for_home_at_path_with_mode(&config_path, codex_home, mode)
}

pub fn ensure_runtime_mem_prodex_observer_for_home_and_root(
    home: &Path,
    prodex_root: &Path,
    prodex_exe: &Path,
) -> Result<PathBuf> {
    let wrapper_path = runtime_mem_prodex_claude_wrapper_path_from_root(prodex_root);
    write_runtime_mem_prodex_claude_wrapper(&wrapper_path, prodex_exe)?;
    let settings_path = runtime_mem_settings_path_from_home(home);
    update_runtime_mem_claude_code_path_setting(&settings_path, &wrapper_path)?;
    Ok(wrapper_path)
}

pub fn runtime_mem_claude_plugin_dir_from_home(home: &Path) -> PathBuf {
    home.join(DEFAULT_CLAUDE_CONFIG_DIR_NAME)
        .join("plugins")
        .join("marketplaces")
        .join(CLAUDE_MEM_PLUGIN_MARKETPLACE_OWNER)
        .join("plugin")
}

pub fn runtime_mem_claude_plugin_manifest_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(".claude-plugin").join("plugin.json")
}

pub fn runtime_mem_data_dir_from_home(home: &Path) -> PathBuf {
    home.join(CLAUDE_MEM_DATA_DIR_NAME)
}

pub fn runtime_mem_settings_path_from_home(home: &Path) -> PathBuf {
    runtime_mem_data_dir_from_home(home).join(CLAUDE_MEM_SETTINGS_FILE_NAME)
}

pub fn runtime_mem_transcript_watch_config_path_from_home(home: &Path) -> PathBuf {
    let settings_path = runtime_mem_settings_path_from_home(home);
    let default_path =
        runtime_mem_data_dir_from_home(home).join(CLAUDE_MEM_TRANSCRIPT_WATCH_FILE_NAME);
    let Some(raw) = fs::read_to_string(&settings_path).ok() else {
        return default_path;
    };
    let Some(settings) = serde_json::from_str::<serde_json::Value>(&raw).ok() else {
        return default_path;
    };
    let flat = settings
        .get("CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH")
        .and_then(serde_json::Value::as_str);
    let nested = settings
        .get("env")
        .and_then(serde_json::Value::as_object)
        .and_then(|env| env.get("CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH"))
        .and_then(serde_json::Value::as_str);
    flat.or(nested).map(PathBuf::from).unwrap_or(default_path)
}

pub fn ensure_runtime_mem_codex_watch_for_home_at_path(
    config_path: &Path,
    codex_home: &Path,
) -> Result<()> {
    ensure_runtime_mem_codex_watch_for_home_at_path_with_mode(
        config_path,
        codex_home,
        RuntimeMemTranscriptMode::Slim,
    )
}

pub fn ensure_runtime_mem_codex_watch_for_home_at_path_with_mode(
    config_path: &Path,
    codex_home: &Path,
    mode: RuntimeMemTranscriptMode,
) -> Result<()> {
    let sessions_root = runtime_mem_codex_sessions_root(codex_home);
    ensure_runtime_mem_codex_watch_for_sessions_root_with_mode(config_path, &sessions_root, mode)
}

pub fn ensure_runtime_mem_codex_watch_for_sessions_root(
    config_path: &Path,
    sessions_root: &Path,
) -> Result<()> {
    ensure_runtime_mem_codex_watch_for_sessions_root_with_mode(
        config_path,
        sessions_root,
        RuntimeMemTranscriptMode::Slim,
    )
}

pub fn ensure_runtime_mem_codex_watch_for_sessions_root_with_mode(
    config_path: &Path,
    sessions_root: &Path,
    mode: RuntimeMemTranscriptMode,
) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw = fs::read_to_string(config_path).ok();
    let mut config = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !config.is_object() {
        config = serde_json::json!({});
    }

    let object = config
        .as_object_mut()
        .expect("transcript watch config should be normalized to an object");
    object.insert("version".to_string(), serde_json::json!(1));
    if !object
        .get("stateFile")
        .is_some_and(serde_json::Value::is_string)
    {
        let state_file = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(CLAUDE_MEM_TRANSCRIPT_WATCH_STATE_FILE_NAME);
        object.insert(
            "stateFile".to_string(),
            serde_json::json!(state_file.display().to_string()),
        );
    }

    let schemas = object
        .entry("schemas".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !schemas.is_object() {
        *schemas = serde_json::json!({});
    }
    schemas
        .as_object_mut()
        .expect("transcript watch schemas should be an object")
        .insert(
            CLAUDE_MEM_CODEX_SCHEMA_NAME.to_string(),
            runtime_mem_codex_schema_for_mode(mode),
        );

    let watch_glob = runtime_mem_codex_watch_glob(sessions_root);
    let watch_name = runtime_mem_prodex_watch_name(sessions_root);
    let desired_watch = runtime_mem_codex_watch_definition(&watch_name, &watch_glob);

    let watches = object
        .entry("watches".to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !watches.is_array() {
        *watches = serde_json::json!([]);
    }
    let watches = watches
        .as_array_mut()
        .expect("transcript watches should be an array");

    if watches.iter().any(|watch| {
        watch.get("schema").and_then(serde_json::Value::as_str)
            == Some(CLAUDE_MEM_CODEX_SCHEMA_NAME)
            && watch.get("path").and_then(serde_json::Value::as_str) == Some(watch_glob.as_str())
    }) {
        let rendered = serde_json::to_string_pretty(&config)
            .context("failed to render claude-mem transcript watch config")?;
        fs::write(config_path, format!("{rendered}\n"))
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        return Ok(());
    }

    if let Some(existing) = watches.iter_mut().find(|watch| {
        watch.get("name").and_then(serde_json::Value::as_str) == Some(watch_name.as_str())
    }) {
        *existing = desired_watch;
    } else {
        watches.push(desired_watch);
    }

    let rendered = serde_json::to_string_pretty(&config)
        .context("failed to render claude-mem transcript watch config")?;
    fs::write(config_path, format!("{rendered}\n"))
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

pub fn runtime_mem_prodex_claude_wrapper_path_from_root(prodex_root: &Path) -> PathBuf {
    let file_name = if cfg!(windows) {
        format!("{PRODEX_CLAUDE_MEM_WRAPPER_NAME}.cmd")
    } else {
        PRODEX_CLAUDE_MEM_WRAPPER_NAME.to_string()
    };
    prodex_root.join(PRODEX_CLAUDE_MEM_DIR_NAME).join(file_name)
}

pub fn runtime_mem_default_codex_schema() -> serde_json::Value {
    runtime_mem_slim_codex_schema()
}

pub fn runtime_mem_full_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.3",
        "description": "Full schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            { "name": "session-meta", "match": { "path": "type", "equals": "session_meta" }, "action": "session_context", "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" } },
            { "name": "turn-context", "match": { "path": "type", "equals": "turn_context" }, "action": "session_context", "fields": { "cwd": "payload.cwd" } },
            { "name": "user-message", "match": { "path": "payload.type", "equals": "user_message" }, "action": "session_init", "fields": { "prompt": "payload.message" } },
            { "name": "assistant-message", "match": { "path": "payload.type", "equals": "agent_message" }, "action": "assistant_message", "fields": { "message": "payload.message" } },
            {
                "name": "tool-use",
                "match": { "path": "payload.type", "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command"] },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": { "coalesce": ["payload.name", "payload.type", { "value": "web_search" }] },
                    "toolInput": { "coalesce": ["payload.arguments", "payload.input", "payload.command", "payload.action"] }
                }
            },
            {
                "name": "tool-result",
                "match": { "path": "payload.type", "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"] },
                "action": "tool_result",
                "fields": { "toolId": "payload.call_id", "toolResponse": "payload.output" }
            },
            { "name": "session-end", "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed"] }, "action": "session_end" }
        ]
    })
}

pub fn runtime_mem_super_slim_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.6-super-slim",
        "description": "Super-slim schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            { "name": "session-meta", "match": { "path": "type", "equals": "session_meta" }, "action": "session_context", "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" } },
            { "name": "turn-context", "match": { "path": "type", "equals": "turn_context" }, "action": "session_context", "fields": { "cwd": "payload.cwd" } },
            {
                "name": "user-message",
                "match": { "path": "payload.type", "equals": "user_message" },
                "action": "session_init",
                "fields": {
                    "prompt": {
                        "coalesce": [
                            "payload.prompt_summary",
                            "payload.metadata.prompt_summary",
                            "payload.metadata.artifact_ref",
                            "payload.metadata.artifact_id",
                            "payload.metadata.artifactId",
                            "payload.artifact.reference",
                            "payload.artifact.ref",
                            "payload.artifact.id",
                            "payload.artifact_id",
                            "payload.artifactId",
                            { "value": "user prompt recorded by prodex super-slim mem; content omitted" }
                        ]
                    }
                }
            },
            {
                "name": "assistant-message",
                "match": { "path": "payload.type", "equals": "agent_message" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.summary",
                            "payload.title",
                            { "value": "assistant response recorded by prodex super-slim mem" }
                        ]
                    }
                }
            },
            {
                "name": "tool-use",
                "match": { "path": "payload.type", "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command"] },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": { "coalesce": ["payload.name", "payload.type", { "value": "web_search" }] },
                    "toolInput": { "coalesce": ["payload.command", "payload.action", "payload.name", { "value": "tool call" }] }
                }
            },
            {
                "name": "tool-result",
                "match": { "path": "payload.type", "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"] },
                "action": "tool_result",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolResponse": {
                        "coalesce": [
                            "payload.summary",
                            "payload.metadata.summary",
                            "payload.metadata.artifact_ref",
                            "payload.metadata.artifact_id",
                            "payload.metadata.artifactId",
                            "payload.artifact.reference",
                            "payload.artifact.ref",
                            "payload.artifact.id",
                            "payload.artifact_id",
                            "payload.artifactId",
                            { "value": "tool result recorded by prodex super-slim mem; output omitted" }
                        ]
                    }
                }
            },
            { "name": "session-end", "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed"] }, "action": "session_end" }
        ]
    })
}

fn write_runtime_mem_prodex_claude_wrapper(wrapper_path: &Path, prodex_exe: &Path) -> Result<()> {
    if let Some(parent) = wrapper_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let contents = if cfg!(windows) {
        format!(
            "@echo off\r\n\"{}\" claude --skip-quota-check -- %*\r\n",
            prodex_exe.display()
        )
    } else {
        format!(
            "#!/bin/sh\nexec {} claude --skip-quota-check -- \"$@\"\n",
            shell_single_quote_path(prodex_exe)
        )
    };
    fs::write(wrapper_path, contents)
        .with_context(|| format!("failed to write {}", wrapper_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(wrapper_path)
            .with_context(|| format!("failed to inspect {}", wrapper_path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(wrapper_path, permissions)
            .with_context(|| format!("failed to chmod {}", wrapper_path.display()))?;
    }
    Ok(())
}

fn update_runtime_mem_claude_code_path_setting(
    settings_path: &Path,
    wrapper_path: &Path,
) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw = fs::read_to_string(settings_path).ok();
    let mut settings = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    settings
        .as_object_mut()
        .expect("claude-mem settings should be normalized to an object")
        .insert(
            CLAUDE_MEM_CLAUDE_CODE_PATH_SETTING.to_string(),
            serde_json::json!(wrapper_path.display().to_string()),
        );

    let rendered =
        serde_json::to_string_pretty(&settings).context("failed to render claude-mem settings")?;
    fs::write(settings_path, format!("{rendered}\n"))
        .with_context(|| format!("failed to write {}", settings_path.display()))?;
    Ok(())
}

fn shell_single_quote_path(path: &Path) -> String {
    let raw = path.display().to_string();
    format!("'{}'", raw.replace('\'', "'\"'\"'"))
}

fn runtime_mem_codex_sessions_root(codex_home: &Path) -> PathBuf {
    let sessions_root = codex_home.join("sessions");
    fs::canonicalize(&sessions_root).unwrap_or(sessions_root)
}

fn runtime_mem_prodex_watch_name(sessions_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    sessions_root.hash(&mut hasher);
    format!(
        "{CLAUDE_MEM_PRODEX_WATCH_NAME_PREFIX}{:016x}",
        hasher.finish()
    )
}

fn runtime_mem_codex_watch_glob(sessions_root: &Path) -> String {
    let mut root = sessions_root.display().to_string();
    while root.ends_with(std::path::MAIN_SEPARATOR) {
        root.pop();
    }
    let sep = std::path::MAIN_SEPARATOR;
    format!("{root}{sep}**{sep}*.jsonl")
}

fn runtime_mem_codex_watch_definition(name: &str, path: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "path": path,
        "schema": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "startAtEnd": true,
        "context": {
            "mode": "agents",
            "updateOn": ["session_start", "session_end"],
        }
    })
}

pub fn runtime_mem_codex_schema_for_mode(mode: RuntimeMemTranscriptMode) -> serde_json::Value {
    match mode {
        RuntimeMemTranscriptMode::Slim => runtime_mem_slim_codex_schema(),
        RuntimeMemTranscriptMode::SuperSlim => runtime_mem_super_slim_codex_schema(),
        RuntimeMemTranscriptMode::Full => runtime_mem_full_codex_schema(),
    }
}

#[derive(Debug, Clone, Copy)]
struct RuntimeMemEventContentSpec {
    content_path: &'static str,
    summary_paths: &'static [&'static str],
    artifact_ref_paths: &'static [&'static str],
}

#[derive(Debug, Clone)]
struct RuntimeMemSeenEventContent {
    original_id: String,
    artifact_ref: Option<String>,
}

#[derive(Debug, Default)]
struct RuntimeMemEventDedupeState {
    seen_content: HashMap<String, RuntimeMemSeenEventContent>,
    seen_assistant_summary: HashMap<String, RuntimeMemSeenEventContent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMemDedupeReplacement {
    summary: String,
    artifact_ref: Option<String>,
}

impl RuntimeMemEventDedupeState {
    fn replacement_for_optional_content(
        &mut self,
        id: String,
        content: &str,
        artifact_ref: Option<String>,
    ) -> Option<RuntimeMemDedupeReplacement> {
        runtime_mem_replacement_for_optional_seen(&mut self.seen_content, id, content, artifact_ref)
    }

    fn replacement_for_optional_assistant_summary(
        &mut self,
        id: String,
        summary: &str,
    ) -> Option<RuntimeMemDedupeReplacement> {
        runtime_mem_replacement_for_optional_seen(
            &mut self.seen_assistant_summary,
            id,
            summary,
            runtime_mem_prodex_artifact_ref(None, summary),
        )
    }
}

fn runtime_mem_replacement_for_optional_seen(
    seen_by_content: &mut HashMap<String, RuntimeMemSeenEventContent>,
    id: String,
    content: &str,
    artifact_ref: Option<String>,
) -> Option<RuntimeMemDedupeReplacement> {
    if let Some(seen) = seen_by_content.get_mut(content) {
        if seen.artifact_ref.is_none() {
            seen.artifact_ref = artifact_ref.clone();
        }
        let content_hash = runtime_mem_content_hash(content);
        if let Some(artifact_ref) = artifact_ref.or_else(|| seen.artifact_ref.clone()) {
            return Some(RuntimeMemDedupeReplacement {
                summary: runtime_mem_artifact_recall_summary(
                    &artifact_ref,
                    &content_hash,
                    content.len(),
                ),
                artifact_ref: Some(artifact_ref),
            });
        }
        return Some(RuntimeMemDedupeReplacement {
            summary: runtime_mem_duplicate_recall_summary(
                &seen.original_id,
                &content_hash,
                content.len(),
            ),
            artifact_ref: None,
        });
    }

    seen_by_content.insert(
        content.to_string(),
        RuntimeMemSeenEventContent {
            original_id: id,
            artifact_ref,
        },
    );
    None
}

fn runtime_mem_super_slim_shadow_codex_event_with_dedupe(
    event: &Value,
    index: usize,
    dedupe_state: &mut RuntimeMemEventDedupeState,
) -> Value {
    let spec = runtime_mem_event_content_spec(event);
    let replacement = spec.and_then(|spec| {
        let content_replacement = runtime_mem_lookup_json_path(event, spec.content_path)
            .and_then(Value::as_str)
            .and_then(|content| {
                let artifact_ref = runtime_mem_first_prodex_artifact_ref_at_paths(
                    event,
                    spec.artifact_ref_paths,
                    content,
                );
                dedupe_state.replacement_for_optional_content(
                    runtime_mem_event_dedupe_id(event, index),
                    content,
                    artifact_ref,
                )
            });
        content_replacement
            .or_else(|| {
                runtime_mem_assistant_summary_duplicate_replacement(event, index, dedupe_state)
            })
            .map(|replacement| (spec, replacement))
    });
    let mut shadow = runtime_mem_super_slim_shadow_codex_event(event);
    if let Some((spec, replacement)) = replacement {
        for path in spec.summary_paths {
            runtime_mem_set_json_path(
                &mut shadow,
                path,
                Value::String(replacement.summary.clone()),
            );
        }
        if let Some(artifact_ref) = replacement.artifact_ref {
            runtime_mem_set_json_path(
                &mut shadow,
                "payload.metadata.artifact_ref",
                Value::String(artifact_ref),
            );
        }
    }
    shadow
}

fn runtime_mem_assistant_summary_duplicate_replacement(
    event: &Value,
    index: usize,
    dedupe_state: &mut RuntimeMemEventDedupeState,
) -> Option<RuntimeMemDedupeReplacement> {
    let payload_type = runtime_mem_lookup_json_path(event, "payload.type")?.as_str()?;
    if payload_type != "agent_message" {
        return None;
    }
    let summary = runtime_mem_lookup_json_path(event, "payload.summary")?
        .as_str()?
        .trim();
    if summary.is_empty() {
        return None;
    }
    dedupe_state.replacement_for_optional_assistant_summary(
        runtime_mem_event_dedupe_id(event, index),
        summary,
    )
}

fn runtime_mem_event_content_spec(event: &Value) -> Option<RuntimeMemEventContentSpec> {
    let payload_type = runtime_mem_lookup_json_path(event, "payload.type")?.as_str()?;
    match payload_type {
        "user_message" => Some(RuntimeMemEventContentSpec {
            content_path: "payload.message",
            summary_paths: RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS,
        }),
        "agent_message" => Some(RuntimeMemEventContentSpec {
            content_path: "payload.message",
            summary_paths: RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
        }),
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
            Some(RuntimeMemEventContentSpec {
                content_path: "payload.output",
                summary_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS,
                artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
            })
        }
        _ => None,
    }
}

fn runtime_mem_event_dedupe_id(event: &Value, index: usize) -> String {
    runtime_mem_first_text_at_paths(event, &["payload.call_id", "payload.id", "id"])
        .unwrap_or_else(|| format!("event[{index}]"))
}

fn runtime_mem_lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn runtime_mem_shadow_user_message(event: &mut Value) {
    let summary =
        runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS)
            .or_else(|| {
                runtime_mem_shadow_summary_for_path(
                    event,
                    "payload.message",
                    "user prompt",
                    "prompt",
                )
            });
    let artifact_ref =
        runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS)
            .or_else(|| runtime_mem_extract_artifact_marker(event));

    if let Some(summary) = summary {
        runtime_mem_set_json_path(
            event,
            "payload.prompt_summary",
            Value::String(summary.clone()),
        );
        runtime_mem_set_json_path(
            event,
            "payload.metadata.prompt_summary",
            Value::String(summary),
        );
    }
    if let Some(artifact_ref) = artifact_ref {
        runtime_mem_set_json_path(
            event,
            "payload.metadata.artifact_ref",
            Value::String(artifact_ref),
        );
    }
    if runtime_mem_lookup_json_path(event, "payload.message").is_some() {
        runtime_mem_set_json_path(
            event,
            "payload.message",
            Value::String(
                "user prompt shadowed by prodex super-slim mem; full content omitted".to_string(),
            ),
        );
    }
}

fn runtime_mem_shadow_assistant_message(event: &mut Value) {
    if runtime_mem_lookup_json_path(event, "payload.summary").is_none()
        && let Some(summary) = runtime_mem_shadow_summary_for_path(
            event,
            "payload.message",
            "assistant response",
            "message",
        )
    {
        runtime_mem_set_json_path(event, "payload.summary", Value::String(summary));
    }
    if runtime_mem_lookup_json_path(event, "payload.message").is_some() {
        runtime_mem_set_json_path(
            event,
            "payload.message",
            Value::String(
                "assistant response shadowed by prodex super-slim mem; full content omitted"
                    .to_string(),
            ),
        );
    }
}

fn runtime_mem_shadow_tool_output(event: &mut Value) {
    let summary = runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS)
        .or_else(|| {
            runtime_mem_shadow_summary_for_path(event, "payload.output", "tool output", "output")
        });
    let artifact_ref =
        runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS)
            .or_else(|| runtime_mem_extract_artifact_marker(event));

    if let Some(summary) = summary {
        runtime_mem_set_json_path(event, "payload.summary", Value::String(summary.clone()));
        runtime_mem_set_json_path(event, "payload.metadata.summary", Value::String(summary));
    }
    if let Some(artifact_ref) = artifact_ref {
        runtime_mem_set_json_path(
            event,
            "payload.metadata.artifact_ref",
            Value::String(artifact_ref),
        );
    }
    if runtime_mem_lookup_json_path(event, "payload.output").is_some() {
        runtime_mem_set_json_path(
            event,
            "payload.output",
            Value::String(
                "tool output shadowed by prodex super-slim mem; full content omitted".to_string(),
            ),
        );
    }
}

fn runtime_mem_shadow_summary_for_path(
    event: &Value,
    path: &str,
    label: &str,
    omitted_name: &str,
) -> Option<String> {
    let text = runtime_mem_lookup_json_path(event, path)?.as_str()?;
    let artifact_ref = runtime_mem_extract_artifact_marker(event);
    Some(runtime_mem_shadow_summary_from_text(
        text,
        label,
        omitted_name,
        artifact_ref.as_deref(),
    ))
}

fn runtime_mem_shadow_summary_from_text(
    text: &str,
    label: &str,
    omitted_name: &str,
    artifact_ref: Option<&str>,
) -> String {
    let prefix_limit = runtime_mem_shadow_summary_prefix_char_limit(artifact_ref);
    let first_line = runtime_mem_first_useful_line(text)
        .map(|line| runtime_mem_truncate_chars(line, prefix_limit))
        .unwrap_or_else(|| "(empty)".to_string());
    let ref_part = artifact_ref
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("; ref={value}"))
        .unwrap_or_default();
    format!(
        "{label} summary: {first_line} [bytes={}; approx_tokens={}; full {omitted_name} omitted{ref_part}]",
        text.len(),
        runtime_mem_approx_token_count(text),
    )
}

fn runtime_mem_shadow_summary_prefix_char_limit(artifact_ref: Option<&str>) -> usize {
    if artifact_ref.is_some_and(|value| runtime_mem_normalize_prodex_artifact_ref(value).is_some())
    {
        RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT
    } else {
        RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT
    }
}

fn runtime_mem_first_useful_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn runtime_mem_truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn runtime_mem_approx_token_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn runtime_mem_first_text_at_paths(event: &Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        runtime_mem_lookup_json_path(event, path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

fn runtime_mem_first_prodex_artifact_ref_at_paths(
    event: &Value,
    paths: &[&str],
    content: &str,
) -> Option<String> {
    paths
        .iter()
        .find_map(|path| {
            runtime_mem_lookup_json_path(event, path)
                .and_then(Value::as_str)
                .and_then(runtime_mem_normalize_prodex_artifact_ref)
        })
        .or_else(|| runtime_mem_prodex_artifact_ref(None, content))
}

fn runtime_mem_prodex_artifact_ref(artifact_ref: Option<&str>, content: &str) -> Option<String> {
    artifact_ref
        .and_then(runtime_mem_normalize_prodex_artifact_ref)
        .or_else(|| runtime_mem_extract_artifact_marker_from_text(content))
}

fn runtime_mem_normalize_prodex_artifact_ref(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.starts_with("prodex-artifact:") || text.starts_with("prodex://artifact/") {
        return Some(text.to_string());
    }
    runtime_mem_extract_artifact_marker_from_text(text)
}

fn runtime_mem_artifact_recall_summary(
    artifact_ref: &str,
    content_hash: &str,
    original_bytes: usize,
) -> String {
    format!(
        "{artifact_ref} [mem art; content_hash={content_hash}; original_bytes={original_bytes}]"
    )
}

fn runtime_mem_duplicate_recall_summary(
    original_id: &str,
    content_hash: &str,
    original_bytes: usize,
) -> String {
    format!(
        "mem dup: original={original_id}; content_hash={content_hash}; original_bytes={original_bytes}"
    )
}

fn runtime_mem_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn runtime_mem_extract_artifact_marker(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => runtime_mem_extract_artifact_marker_from_text(text),
        Value::Array(values) => values.iter().find_map(runtime_mem_extract_artifact_marker),
        Value::Object(object) => object
            .values()
            .find_map(runtime_mem_extract_artifact_marker),
        _ => None,
    }
}

fn runtime_mem_extract_artifact_marker_from_text(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|part| part.trim_matches(|c: char| c.is_ascii_punctuation() && c != ':' && c != '/'))
        .find(|part| part.starts_with("prodex-artifact:") || part.starts_with("prodex://artifact/"))
        .map(str::to_string)
}

fn runtime_mem_set_json_path(value: &mut Value, path: &str, new_value: Value) {
    let mut current = value;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if let Value::Object(object) = current {
                object.insert(part.to_string(), new_value);
            }
            return;
        }
        if !current.is_object() {
            *current = serde_json::json!({});
        }
        let object = current
            .as_object_mut()
            .expect("json path container should be object");
        current = object
            .entry(part.to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
}

fn runtime_mem_value_is_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn runtime_mem_value_contains_artifact_marker(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            text.contains("prodex-artifact:")
                || text.contains("prodex://artifact/")
                || text.contains("prodex smart context artifact")
        }
        Value::Array(values) => values
            .iter()
            .any(runtime_mem_value_contains_artifact_marker),
        Value::Object(object) => object
            .values()
            .any(runtime_mem_value_contains_artifact_marker),
        _ => false,
    }
}

fn runtime_mem_capsule_is_project_local(
    capsule: &RuntimeMemCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
) -> bool {
    let (Some(project_root), Some(capsule_path)) = (&context.project_root, &capsule.project_path)
    else {
        return false;
    };
    let project_root = runtime_mem_normalized_path(project_root);
    let capsule_path = runtime_mem_normalized_path(capsule_path);
    capsule_path == project_root || capsule_path.starts_with(project_root)
}

fn runtime_mem_capsule_is_recent(
    capsule: &RuntimeMemCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
) -> bool {
    let (Some(updated_at), Some(now)) = (capsule.updated_at_seconds, context.now_seconds) else {
        return false;
    };
    if updated_at >= now {
        return true;
    }
    now.checked_sub(updated_at)
        .is_some_and(|age| (age as u64) <= context.recent_window_seconds)
}

fn runtime_mem_capsule_order(
    left: &(RuntimeMemCapsuleMetadata, RuntimeMemCapsulePriority),
    right: &(RuntimeMemCapsuleMetadata, RuntimeMemCapsulePriority),
) -> Ordering {
    runtime_mem_capsule_priority_rank(left.1)
        .cmp(&runtime_mem_capsule_priority_rank(right.1))
        .then_with(|| runtime_mem_relevance_order(right.0.relevance, left.0.relevance))
        .then_with(|| {
            right
                .0
                .updated_at_seconds
                .unwrap_or(i64::MIN)
                .cmp(&left.0.updated_at_seconds.unwrap_or(i64::MIN))
        })
        .then_with(|| left.0.token_cost.cmp(&right.0.token_cost))
        .then_with(|| left.0.id.cmp(&right.0.id))
}

fn runtime_mem_recall_diet_capsule_order(
    left: &(
        RuntimeMemRecallCapsuleMetadata,
        RuntimeMemCapsulePriority,
        usize,
    ),
    right: &(
        RuntimeMemRecallCapsuleMetadata,
        RuntimeMemCapsulePriority,
        usize,
    ),
) -> Ordering {
    runtime_mem_recall_diet_rank(left.1, left.2)
        .cmp(&runtime_mem_recall_diet_rank(right.1, right.2))
        .then_with(|| right.2.cmp(&left.2))
        .then_with(|| {
            runtime_mem_relevance_order(right.0.capsule.relevance, left.0.capsule.relevance)
        })
        .then_with(|| {
            right
                .0
                .capsule
                .updated_at_seconds
                .unwrap_or(i64::MIN)
                .cmp(&left.0.capsule.updated_at_seconds.unwrap_or(i64::MIN))
        })
        .then_with(|| left.0.capsule.token_cost.cmp(&right.0.capsule.token_cost))
        .then_with(|| left.0.capsule.id.cmp(&right.0.capsule.id))
}

fn runtime_mem_recall_diet_rank(priority: RuntimeMemCapsulePriority, intent_score: usize) -> u8 {
    match (priority, intent_score > 0) {
        (RuntimeMemCapsulePriority::Required, _) => 0,
        (RuntimeMemCapsulePriority::ProjectLocal, true) => 1,
        (RuntimeMemCapsulePriority::Recent, true) => 2,
        (RuntimeMemCapsulePriority::Optional, true) => 3,
        (RuntimeMemCapsulePriority::ProjectLocal, false) => 4,
        (RuntimeMemCapsulePriority::Recent, false) => 5,
        (RuntimeMemCapsulePriority::Optional, false) => 6,
    }
}

fn runtime_mem_capsule_priority_rank(priority: RuntimeMemCapsulePriority) -> u8 {
    match priority {
        RuntimeMemCapsulePriority::Required => 0,
        RuntimeMemCapsulePriority::ProjectLocal => 1,
        RuntimeMemCapsulePriority::Recent => 2,
        RuntimeMemCapsulePriority::Optional => 3,
    }
}

fn runtime_mem_relevance_order(left: f32, right: f32) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

fn runtime_mem_capsule_intent_score(
    capsule: &RuntimeMemRecallCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
    intent: &RuntimeMemRecallIntent,
) -> usize {
    let path_score = intent
        .paths
        .iter()
        .filter(|intent_path| {
            runtime_mem_capsule_matches_intent_path(capsule, context, intent_path)
        })
        .count();
    let symbol_score = intent
        .symbols
        .iter()
        .filter(|intent_symbol| {
            capsule.symbols.iter().any(|capsule_symbol| {
                runtime_mem_symbols_match(capsule_symbol.as_str(), intent_symbol.as_str())
            })
        })
        .count();

    path_score.saturating_add(symbol_score)
}

fn runtime_mem_capsule_matches_intent_path(
    capsule: &RuntimeMemRecallCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
    intent_path: &Path,
) -> bool {
    let intent_path =
        runtime_mem_intent_path_for_matching(intent_path, context.project_root.as_deref());
    capsule
        .capsule
        .project_path
        .iter()
        .chain(capsule.paths.iter())
        .map(|path| runtime_mem_intent_path_for_matching(path, context.project_root.as_deref()))
        .any(|capsule_path| runtime_mem_paths_overlap(&capsule_path, &intent_path))
}

fn runtime_mem_intent_path_for_matching(path: &Path, project_root: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        return runtime_mem_normalized_path(path);
    }
    project_root
        .map(|root| runtime_mem_normalized_path(&root.join(path)))
        .unwrap_or_else(|| runtime_mem_normalized_path(path))
}

fn runtime_mem_paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn runtime_mem_symbols_match(capsule_symbol: &str, intent_symbol: &str) -> bool {
    let capsule_symbol = runtime_mem_normalized_symbol(capsule_symbol);
    let intent_symbol = runtime_mem_normalized_symbol(intent_symbol);
    if capsule_symbol.is_empty() || intent_symbol.is_empty() {
        return false;
    }
    capsule_symbol == intent_symbol
        || capsule_symbol.ends_with(&format!("::{intent_symbol}"))
        || capsule_symbol.ends_with(&format!(".{intent_symbol}"))
        || intent_symbol.ends_with(&format!("::{capsule_symbol}"))
        || intent_symbol.ends_with(&format!(".{capsule_symbol}"))
}

fn runtime_mem_normalized_symbol(symbol: &str) -> String {
    symbol.trim().trim_matches('`').to_ascii_lowercase()
}

fn runtime_mem_normalized_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn runtime_mem_slim_codex_schema() -> serde_json::Value {
    serde_json::json!({
        "name": CLAUDE_MEM_CODEX_SCHEMA_NAME,
        "version": "0.4-slim",
        "description": "Slim schema for Codex session JSONL files under ~/.codex/sessions.",
        "events": [
            { "name": "session-meta", "match": { "path": "type", "equals": "session_meta" }, "action": "session_context", "fields": { "sessionId": "payload.id", "cwd": "payload.cwd" } },
            { "name": "turn-context", "match": { "path": "type", "equals": "turn_context" }, "action": "session_context", "fields": { "cwd": "payload.cwd" } },
            { "name": "user-message", "match": { "path": "payload.type", "equals": "user_message" }, "action": "session_init", "fields": { "prompt": "payload.message" } },
            {
                "name": "assistant-message",
                "match": { "path": "payload.type", "equals": "agent_message" },
                "action": "assistant_message",
                "fields": {
                    "message": {
                        "coalesce": [
                            "payload.summary",
                            "payload.title",
                            { "value": "assistant response recorded by prodex slim mem" }
                        ]
                    }
                }
            },
            {
                "name": "tool-use",
                "match": { "path": "payload.type", "in": ["function_call", "custom_tool_call", "web_search_call", "exec_command"] },
                "action": "tool_use",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolName": { "coalesce": ["payload.name", "payload.type", { "value": "web_search" }] },
                    "toolInput": { "coalesce": ["payload.command", "payload.action", "payload.name", { "value": "tool call" }] }
                }
            },
            {
                "name": "tool-result",
                "match": { "path": "payload.type", "in": ["function_call_output", "custom_tool_call_output", "exec_command_output"] },
                "action": "tool_result",
                "fields": {
                    "toolId": "payload.call_id",
                    "toolResponse": {
                        "coalesce": [
                            "payload.summary",
                            "payload.metadata.summary",
                            { "value": "tool result recorded by prodex slim mem; output omitted" }
                        ]
                    }
                }
            },
            { "name": "session-end", "match": { "path": "payload.type", "in": ["turn_aborted", "turn_completed"] }, "action": "session_end" }
        ]
    })
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
