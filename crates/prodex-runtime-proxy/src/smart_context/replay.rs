use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SMART_CONTEXT_REPLAY_CORPUS_SCHEMA_VERSION: u32 = 2;
pub const SMART_CONTEXT_REPLAY_REPORT_SCHEMA_VERSION: u32 = 3;
pub const SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUPS: usize = 64;
pub const SMART_CONTEXT_REPLAY_MAX_SCENARIOS_PER_CONCURRENT_GROUP: usize = 64;
pub const SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUP_NAME_BYTES: usize = 128;
pub const SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP: usize = 2 * 1024 * 1024;
pub const SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES: usize = 8 * 1024 * 1024;
pub const SMART_CONTEXT_REPLAY_MAX_ARTIFACT_REFERENCES: usize = 1024;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartContextReplayTurnInput {
    pub request: serde_json::Value,
    #[serde(default)]
    pub required_text: Vec<String>,
    #[serde(default)]
    pub preserve_json_pointers: Vec<String>,
    pub expected_outcome: SmartContextReplayExpectedOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    if text.len() > SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES {
        return Err(format!(
            "Smart Context replay corpus exceeds {} input bytes before parsing",
            SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES
        ));
    }
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
    let serialized_bytes = serde_json::to_vec(corpus)
        .map_err(|error| format!("Smart Context replay corpus serialization failed: {error}"))?
        .len();
    if serialized_bytes > SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES {
        return Err(format!(
            "Smart Context replay corpus exceeds {} total serialized bytes before execution (would use {})",
            SMART_CONTEXT_REPLAY_MAX_TOTAL_SERIALIZED_BYTES, serialized_bytes
        ));
    }
    let mut ids = BTreeSet::new();
    let mut concurrent_groups = BTreeMap::<String, SmartContextReplayConcurrentGroupUsage>::new();
    let mut artifact_references = 0;
    for scenario in &corpus.scenarios {
        smart_context_validate_replay_scenario(
            scenario,
            &mut ids,
            &mut concurrent_groups,
            &mut artifact_references,
        )?;
    }
    if let Some((group, _)) = concurrent_groups
        .iter()
        .find(|(_, usage)| usage.scenarios < 2)
    {
        return Err(format!(
            "Smart Context replay concurrent group {group} needs at least two scenarios"
        ));
    }
    Ok(())
}

#[derive(Default)]
struct SmartContextReplayConcurrentGroupUsage {
    scenarios: usize,
    serialized_bytes: usize,
}

fn smart_context_validate_replay_scenario(
    scenario: &SmartContextReplayScenarioInput,
    ids: &mut BTreeSet<String>,
    concurrent_groups: &mut BTreeMap<String, SmartContextReplayConcurrentGroupUsage>,
    artifact_references: &mut usize,
) -> Result<(), String> {
    smart_context_validate_replay_scenario_shape(scenario, ids)?;
    smart_context_validate_replay_concurrent_group(scenario, concurrent_groups)?;
    smart_context_validate_replay_turns(scenario, artifact_references)
}

fn smart_context_validate_replay_scenario_shape(
    scenario: &SmartContextReplayScenarioInput,
    ids: &mut BTreeSet<String>,
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
    Ok(())
}

fn smart_context_validate_replay_concurrent_group(
    scenario: &SmartContextReplayScenarioInput,
    concurrent_groups: &mut BTreeMap<String, SmartContextReplayConcurrentGroupUsage>,
) -> Result<(), String> {
    let Some(group) = scenario.concurrent_group.as_deref() else {
        return Ok(());
    };
    if group.trim().is_empty() {
        return Err(format!(
            "Smart Context replay scenario {} has an empty concurrent group",
            scenario.id
        ));
    }
    if group.len() > SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUP_NAME_BYTES {
        return Err(format!(
            "Smart Context replay scenario {} concurrent group exceeds {} bytes",
            scenario.id, SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUP_NAME_BYTES
        ));
    }
    if !concurrent_groups.contains_key(group)
        && concurrent_groups.len() >= SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUPS
    {
        return Err(format!(
            "Smart Context replay exceeds {} concurrent groups",
            SMART_CONTEXT_REPLAY_MAX_CONCURRENT_GROUPS
        ));
    }
    let serialized_bytes = serde_json::to_vec(scenario)
        .map_err(|error| {
            format!(
                "Smart Context replay scenario {} serialization failed: {error}",
                scenario.id
            )
        })?
        .len();
    let usage = concurrent_groups.entry(group.to_string()).or_default();
    if usage.scenarios >= SMART_CONTEXT_REPLAY_MAX_SCENARIOS_PER_CONCURRENT_GROUP {
        return Err(format!(
            "Smart Context replay concurrent group {group} exceeds {} scenarios",
            SMART_CONTEXT_REPLAY_MAX_SCENARIOS_PER_CONCURRENT_GROUP
        ));
    }
    let next_serialized_bytes = usage.serialized_bytes.saturating_add(serialized_bytes);
    if next_serialized_bytes > SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP {
        return Err(format!(
            "Smart Context replay concurrent group {group} exceeds {} serialized bytes at scenario {} (would use {})",
            SMART_CONTEXT_REPLAY_MAX_SERIALIZED_BYTES_PER_CONCURRENT_GROUP,
            scenario.id,
            next_serialized_bytes
        ));
    }
    usage.scenarios += 1;
    usage.serialized_bytes = next_serialized_bytes;
    Ok(())
}

fn smart_context_validate_replay_turns(
    scenario: &SmartContextReplayScenarioInput,
    artifact_references: &mut usize,
) -> Result<(), String> {
    for (index, turn) in scenario.turns.iter().enumerate() {
        let references = smart_context_validate_replay_turn(scenario, turn, index)?;
        let next_references = artifact_references.saturating_add(references);
        if next_references > SMART_CONTEXT_REPLAY_MAX_ARTIFACT_REFERENCES {
            return Err(format!(
                "Smart Context replay scenario {} turn {} exceeds {} artifact/reference count (would use {})",
                scenario.id,
                index + 1,
                SMART_CONTEXT_REPLAY_MAX_ARTIFACT_REFERENCES,
                next_references
            ));
        }
        *artifact_references = next_references;
    }
    Ok(())
}

fn smart_context_validate_replay_turn(
    scenario: &SmartContextReplayScenarioInput,
    turn: &SmartContextReplayTurnInput,
    index: usize,
) -> Result<usize, String> {
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
    Ok(smart_context_replay_artifact_reference_count(&turn.request))
}

fn smart_context_replay_artifact_reference_count(value: &serde_json::Value) -> usize {
    let mut values = vec![value];
    let mut texts = Vec::new();
    while let Some(value) = values.pop() {
        match value {
            serde_json::Value::Array(items) => values.extend(items.iter()),
            serde_json::Value::Object(object) => values.extend(object.values()),
            serde_json::Value::String(text) => texts.push(text.as_str()),
            serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            }
        }
    }
    let aliases = texts
        .iter()
        .flat_map(|text| smart_context_replay_artifact_tokens(text))
        .filter_map(smart_context_replay_artifact_alias)
        .collect::<BTreeSet<_>>();
    let mut count = 0usize;
    for text in texts {
        count = count.saturating_add(
            smart_context_replay_artifact_tokens(text)
                .filter(|token| {
                    smart_context_replay_is_artifact_reference(token)
                        || smart_context_replay_is_artifact_alias_reference(token, &aliases)
                })
                .count(),
        );
    }
    count
}

fn smart_context_replay_artifact_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| character.is_whitespace() || matches!(character, ')' | ']' | '}'))
        .map(|token| {
            token.trim_matches(|character: char| {
                matches!(
                    character,
                    '"' | '\''
                        | '`'
                        | ':'
                        | ';'
                        | '.'
                        | ','
                        | '!'
                        | '?'
                        | '('
                        | '['
                        | '{'
                        | '<'
                        | ')'
                        | ']'
                        | '}'
                        | '>'
                )
            })
        })
        .filter(|token| !token.is_empty())
}

fn smart_context_replay_artifact_alias(token: &str) -> Option<&str> {
    let (alias, reference) = token.split_once('=')?;
    let digits = alias.strip_prefix('@')?;
    if digits.is_empty()
        || !digits.chars().all(|character| character.is_ascii_digit())
        || !smart_context_replay_is_artifact_reference(reference)
    {
        return None;
    }
    Some(alias)
}

fn smart_context_replay_is_artifact_alias_reference(token: &str, aliases: &BTreeSet<&str>) -> bool {
    if token.contains('=') {
        return false;
    }
    let rest = token.strip_prefix('@').unwrap_or_default();
    let digit_len = rest
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .count();
    digit_len > 0 && aliases.contains(&token[..1 + digit_len])
}

fn smart_context_replay_is_artifact_reference(token: &str) -> bool {
    let token = smart_context_replay_artifact_tokens(token)
        .next()
        .unwrap_or_default();
    let (expected_hex_digits, payload) = if let Some(payload) = token.strip_prefix("psc2:") {
        (64, payload)
    } else if let Some(payload) = token.strip_prefix("sc2:") {
        (64, payload)
    } else if let Some(payload) = token.strip_prefix("psc:") {
        (16, payload.strip_prefix("sc:").unwrap_or(payload))
    } else if let Some(payload) = token.strip_prefix("sc:") {
        (16, payload)
    } else if let Some(payload) = token.strip_prefix("prodex-artifact:") {
        if let Some(payload) = payload.strip_prefix("sc2:") {
            (64, payload)
        } else if let Some(payload) = payload.strip_prefix("sc:") {
            (16, payload)
        } else {
            return false;
        }
    } else {
        return false;
    };
    let mut characters = payload.chars();
    if (0..expected_hex_digits).any(|_| {
        !characters
            .next()
            .is_some_and(|character| character.is_ascii_hexdigit())
    }) {
        return false;
    }
    !characters
        .next()
        .is_some_and(|character| character.is_ascii_hexdigit())
}
