use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION: u32 = 2;
pub const SMART_CONTEXT_REPLAY_REPORT_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartContextReplayTransport {
    Http,
    Websocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartContextReplayRoute {
    Responses,
    Compact,
    Standard,
    Websocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartContextReplayMode {
    Active,
    Exact,
    Shadow,
    CanaryOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartContextReplayExpectedOutcome {
    Rewrite,
    PassThrough,
    MissingArtifactFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartContextReplayTurnInput {
    pub request: serde_json::Value,
    #[serde(default)]
    pub required_text: Vec<String>,
    #[serde(default)]
    pub preserve_json_pointers: Vec<String>,
    pub expected_outcome: SmartContextReplayExpectedOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartContextReplayScenarioInput {
    pub id: String,
    pub transport: SmartContextReplayTransport,
    pub route: SmartContextReplayRoute,
    pub provider: String,
    pub model: String,
    pub context_window_tokens: u64,
    #[serde(default)]
    pub observed_context_tokens: Option<u64>,
    pub mode: SmartContextReplayMode,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub concurrent_group: Option<String>,
    #[serde(default)]
    pub restart_before_turns: Vec<usize>,
    pub turns: Vec<SmartContextReplayTurnInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartContextReplayCorpus {
    pub schema_version: u32,
    pub scenarios: Vec<SmartContextReplayScenarioInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayProvenance {
    pub package_version: &'static str,
    pub commit_sha: Option<String>,
    pub os: &'static str,
    pub architecture: &'static str,
    pub rust_toolchain: Option<String>,
    pub tokenizer_source: &'static str,
    pub token_measurement: &'static str,
    pub command: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayTurnResult {
    pub turn: usize,
    pub exact_body_bytes: usize,
    pub optimized_body_bytes: usize,
    pub exact_input_tokens: u64,
    pub optimized_input_tokens: u64,
    pub net_saved_tokens: i64,
    pub token_count_source: &'static str,
    pub tokenizer_family: &'static str,
    pub token_confidence_basis_points: u16,
    pub token_error_bound_tokens: u64,
    pub rewrite_applied: bool,
    pub exact_byte_identity: bool,
    pub valid_json: bool,
    pub required_text_preserved: bool,
    pub protocol_fields_preserved: bool,
    pub unresolved_artifact_references: Vec<String>,
    pub selected_transforms: Vec<String>,
    pub exact_state_mutations: u64,
    pub optimized_state_mutations: u64,
    pub blocked_before_upstream: bool,
    pub missing_artifact_count: usize,
    pub validation_passed: bool,
    pub fallback_reason: Option<&'static str>,
    pub allocation_bytes: Option<u64>,
    pub rewrite_duration_ns: u64,
    pub exact_body_sha256: String,
    pub optimized_body_sha256: String,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayScenarioResult {
    pub id: String,
    pub transport: SmartContextReplayTransport,
    pub route: SmartContextReplayRoute,
    pub provider: String,
    pub model: String,
    pub context_window_tokens: u64,
    pub mode: SmartContextReplayMode,
    pub tags: Vec<String>,
    pub concurrent_group: Option<String>,
    pub exact_input_tokens: u64,
    pub optimized_input_tokens: u64,
    pub net_saved_tokens: i64,
    pub turns: Vec<SmartContextReplayTurnResult>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmartContextReplayReport {
    pub schema_version: u32,
    pub corpus_schema_version: u32,
    pub evidence_level: &'static str,
    pub provenance: SmartContextReplayProvenance,
    pub scenarios: Vec<SmartContextReplayScenarioResult>,
    pub exact_input_tokens: u64,
    pub optimized_input_tokens: u64,
    pub net_saved_tokens: i64,
    pub passed: bool,
    pub failures: Vec<String>,
}

pub fn smart_context_parse_replay_corpus_json(
    text: &str,
) -> Result<SmartContextReplayCorpus, String> {
    let corpus = serde_json::from_str::<SmartContextReplayCorpus>(text)
        .map_err(|error| error.to_string())?;
    smart_context_validate_replay_corpus(&corpus)?;
    Ok(corpus)
}

fn smart_context_validate_replay_corpus(corpus: &SmartContextReplayCorpus) -> Result<(), String> {
    if corpus.schema_version != SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported Smart Context replay corpus schema {}, expected {}",
            corpus.schema_version, SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION
        ));
    }
    if corpus.scenarios.is_empty() {
        return Err("Smart Context replay corpus has no scenarios".to_string());
    }
    let mut ids = BTreeSet::new();
    let mut concurrent_groups = std::collections::BTreeMap::<String, usize>::new();
    for scenario in &corpus.scenarios {
        smart_context_validate_replay_scenario(scenario, &mut ids, &mut concurrent_groups)?;
    }
    if let Some((group, _)) = concurrent_groups.iter().find(|(_, count)| **count < 2) {
        return Err(format!(
            "Smart Context replay concurrent group {group} needs at least two scenarios"
        ));
    }
    Ok(())
}

fn smart_context_validate_replay_scenario(
    scenario: &SmartContextReplayScenarioInput,
    ids: &mut BTreeSet<String>,
    concurrent_groups: &mut std::collections::BTreeMap<String, usize>,
) -> Result<(), String> {
    if scenario.id.trim().is_empty() {
        return Err("Smart Context replay scenario id is empty".to_string());
    }
    if !ids.insert(scenario.id.clone()) {
        return Err(format!(
            "duplicate Smart Context replay scenario id {}",
            scenario.id
        ));
    }
    if scenario.provider.trim().is_empty() || scenario.model.trim().is_empty() {
        return Err(format!(
            "Smart Context replay scenario {} has empty provider or model",
            scenario.id
        ));
    }
    if scenario.context_window_tokens == 0 || scenario.turns.is_empty() {
        return Err(format!(
            "Smart Context replay scenario {} needs a context window and at least one turn",
            scenario.id
        ));
    }
    if (scenario.transport == SmartContextReplayTransport::Websocket)
        != (scenario.route == SmartContextReplayRoute::Websocket)
    {
        return Err(format!(
            "Smart Context replay scenario {} transport and route disagree",
            scenario.id
        ));
    }
    if scenario
        .restart_before_turns
        .iter()
        .any(|turn| *turn < 2 || *turn > scenario.turns.len())
    {
        return Err(format!(
            "Smart Context replay scenario {} has an invalid restart boundary",
            scenario.id
        ));
    }
    if let Some(group) = scenario.concurrent_group.as_deref() {
        if group.trim().is_empty() {
            return Err(format!(
                "Smart Context replay scenario {} has an empty concurrent group",
                scenario.id
            ));
        }
        *concurrent_groups.entry(group.to_string()).or_default() += 1;
    }
    for (index, turn) in scenario.turns.iter().enumerate() {
        smart_context_validate_replay_turn(scenario, turn, index)?;
    }
    Ok(())
}

fn smart_context_validate_replay_turn(
    scenario: &SmartContextReplayScenarioInput,
    turn: &SmartContextReplayTurnInput,
    index: usize,
) -> Result<(), String> {
    if !turn.request.is_object() {
        return Err(format!(
            "Smart Context replay scenario {} turn {} request must be a JSON object",
            scenario.id,
            index + 1
        ));
    }
    if turn
        .request
        .get("model")
        .and_then(serde_json::Value::as_str)
        != Some(scenario.model.as_str())
    {
        return Err(format!(
            "Smart Context replay scenario {} turn {} model does not match scenario model",
            scenario.id,
            index + 1
        ));
    }
    if turn
        .preserve_json_pointers
        .iter()
        .any(|pointer| !pointer.starts_with('/'))
    {
        return Err(format!(
            "Smart Context replay scenario {} turn {} has an invalid JSON pointer",
            scenario.id,
            index + 1
        ));
    }
    Ok(())
}
