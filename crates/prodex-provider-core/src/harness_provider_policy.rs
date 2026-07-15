//! Evaluation-backed provider/model harness policy.

use crate::{EffectiveHarnessMode, ProviderEndpoint, ProviderId, provider_model_spec};
use serde::Serialize;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HarnessToolAlias {
    pub canonical: &'static str,
    pub provider_native: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessResponsePolicy {
    Disabled,
    RestoreToolAliases,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HarnessProviderPolicySpec {
    pub evaluation_id: &'static str,
    pub evaluation_version: u16,
    pub provider: ProviderId,
    /// Exact model id or `*` for every known model in the provider catalog.
    pub model: &'static str,
    pub reason: &'static str,
    pub minimal_instructions: bool,
    pub native_anthropic_messages: bool,
    pub tool_aliases: &'static [HarnessToolAlias],
    pub response_policy: HarnessResponsePolicy,
    pub continuity_tested: bool,
}

const GEMINI_TOOL_ALIASES: &[HarnessToolAlias] = &[HarnessToolAlias {
    canonical: "exec_command",
    provider_native: "run_shell_command",
}];

pub const HARNESS_PROVIDER_POLICY_CATALOG: &[HarnessProviderPolicySpec] = &[
    HarnessProviderPolicySpec {
        evaluation_id: "gemini-native-tool-alias-roundtrip-v1",
        evaluation_version: 1,
        provider: ProviderId::Gemini,
        model: "*",
        reason: "Gemini models recognize run_shell_command as the native shell-tool name; the canonical name is restored before returning tool calls to Codex.",
        minimal_instructions: false,
        native_anthropic_messages: false,
        tool_aliases: GEMINI_TOOL_ALIASES,
        response_policy: HarnessResponsePolicy::RestoreToolAliases,
        continuity_tested: true,
    },
    HarnessProviderPolicySpec {
        evaluation_id: "anthropic-native-messages-roundtrip-v1",
        evaluation_version: 1,
        provider: ProviderId::Anthropic,
        model: "*",
        reason: "Anthropic models use the native Messages request, response, and SSE contracts while Prodex restores the canonical Responses surface.",
        minimal_instructions: false,
        native_anthropic_messages: true,
        tool_aliases: &[],
        response_policy: HarnessResponsePolicy::Disabled,
        continuity_tested: true,
    },
];

pub const fn harness_provider_policy_catalog() -> &'static [HarnessProviderPolicySpec] {
    HARNESS_PROVIDER_POLICY_CATALOG
}

pub fn harness_provider_policy(
    mode: EffectiveHarnessMode,
    provider: ProviderId,
    model: Option<&str>,
) -> Option<&'static HarnessProviderPolicySpec> {
    if mode != EffectiveHarnessMode::Evaluated {
        return None;
    }
    let model = model?.trim();
    if provider_model_spec(provider, model).is_none() {
        return None;
    }
    HARNESS_PROVIDER_POLICY_CATALOG.iter().find(|policy| {
        policy.provider == provider
            && (policy.model == "*" || policy.model.eq_ignore_ascii_case(model))
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct HarnessBodyTransform<'a> {
    pub body: Cow<'a, [u8]>,
    pub applied: bool,
    pub policy: Option<&'static HarnessProviderPolicySpec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarnessProviderTransformError {
    InvalidJson,
    BodyMustBeObject,
    ToolAliasCollision,
}

impl HarnessProviderTransformError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid-json",
            Self::BodyMustBeObject => "body-must-be-object",
            Self::ToolAliasCollision => "tool-alias-collision",
        }
    }
}

impl std::fmt::Display for HarnessProviderTransformError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJson => "harness provider transform body is not valid JSON",
            Self::BodyMustBeObject => "harness provider transform body must be a JSON object",
            Self::ToolAliasCollision => {
                "harness provider-native tool alias collides with an existing tool name"
            }
        })
    }
}

impl std::error::Error for HarnessProviderTransformError {}

pub fn shape_harness_provider_request<'a>(
    mode: EffectiveHarnessMode,
    provider: ProviderId,
    model: Option<&str>,
    endpoint: ProviderEndpoint,
    body: &'a [u8],
) -> Result<HarnessBodyTransform<'a>, HarnessProviderTransformError> {
    let policy = harness_provider_policy(mode, provider, model);
    if endpoint != ProviderEndpoint::Responses || policy.is_none() {
        return Ok(unchanged(body, policy));
    }
    let policy = policy.expect("checked above");
    let mut value = parse_object(body)?;
    reject_tool_alias_collisions(&value, policy.tool_aliases)?;
    let applied = alias_request_tool_names(&mut value, policy.tool_aliases, false);
    transformed(body, value, applied, policy)
}

pub fn postprocess_harness_provider_response<'a>(
    mode: EffectiveHarnessMode,
    provider: ProviderId,
    model: Option<&str>,
    endpoint: ProviderEndpoint,
    body: &'a [u8],
) -> Result<HarnessBodyTransform<'a>, HarnessProviderTransformError> {
    let policy = harness_provider_policy(mode, provider, model);
    if endpoint != ProviderEndpoint::Responses
        || !policy.is_some_and(|policy| {
            policy.response_policy == HarnessResponsePolicy::RestoreToolAliases
        })
    {
        return Ok(unchanged(body, policy));
    }
    let policy = policy.expect("checked above");
    let mut value = parse_object(body)?;
    let applied = alias_response_tool_names(&mut value, policy.tool_aliases);
    transformed(body, value, applied, policy)
}

pub fn postprocess_harness_provider_stream_event<'a>(
    mode: EffectiveHarnessMode,
    provider: ProviderId,
    model: Option<&str>,
    endpoint: ProviderEndpoint,
    event: &'a [u8],
) -> Result<HarnessBodyTransform<'a>, HarnessProviderTransformError> {
    let policy = harness_provider_policy(mode, provider, model);
    if endpoint != ProviderEndpoint::Responses
        || !policy.is_some_and(|policy| {
            policy.response_policy == HarnessResponsePolicy::RestoreToolAliases
        })
    {
        return Ok(unchanged(event, policy));
    }
    let policy = policy.expect("checked above");
    let text =
        std::str::from_utf8(event).map_err(|_| HarnessProviderTransformError::InvalidJson)?;
    let Some(data_start) = text.find("data:") else {
        return Ok(unchanged(event, Some(policy)));
    };
    let payload_start = data_start + "data:".len();
    let whitespace =
        text[payload_start..].len() - text[payload_start..].trim_start_matches([' ', '\t']).len();
    let json_start = payload_start + whitespace;
    let json_end = text[json_start..]
        .find('\n')
        .map_or(text.len(), |index| json_start + index);
    let payload = &text[json_start..json_end];
    if payload == "[DONE]" {
        return Ok(unchanged(event, Some(policy)));
    }
    let mut value: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| HarnessProviderTransformError::InvalidJson)?;
    if !value.is_object() {
        return Err(HarnessProviderTransformError::BodyMustBeObject);
    }
    if !alias_response_tool_names(&mut value, policy.tool_aliases) {
        return Ok(unchanged(event, Some(policy)));
    }
    let mut body = Vec::with_capacity(event.len());
    body.extend_from_slice(&event[..json_start]);
    body.extend_from_slice(
        serde_json::to_string(&value)
            .expect("serializing an in-memory JSON value cannot fail")
            .as_bytes(),
    );
    body.extend_from_slice(&event[json_end..]);
    Ok(HarnessBodyTransform {
        body: Cow::Owned(body),
        applied: true,
        policy: Some(policy),
    })
}

pub fn harness_provider_native_tool_name<'a>(
    mode: EffectiveHarnessMode,
    provider: ProviderId,
    model: Option<&str>,
    name: &'a str,
) -> Cow<'a, str> {
    alias_tool_name(
        name,
        harness_provider_policy(mode, provider, model)
            .map(|policy| policy.tool_aliases)
            .unwrap_or_default(),
        false,
    )
}

pub fn harness_canonical_tool_name<'a>(
    mode: EffectiveHarnessMode,
    provider: ProviderId,
    model: Option<&str>,
    name: &'a str,
) -> Cow<'a, str> {
    let Some(policy) = harness_provider_policy(mode, provider, model)
        .filter(|policy| policy.response_policy == HarnessResponsePolicy::RestoreToolAliases)
    else {
        return Cow::Borrowed(name);
    };
    alias_tool_name(name, policy.tool_aliases, true)
}

fn parse_object(body: &[u8]) -> Result<serde_json::Value, HarnessProviderTransformError> {
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| HarnessProviderTransformError::InvalidJson)?;
    if !value.is_object() {
        return Err(HarnessProviderTransformError::BodyMustBeObject);
    }
    Ok(value)
}

fn unchanged<'a>(
    body: &'a [u8],
    policy: Option<&'static HarnessProviderPolicySpec>,
) -> HarnessBodyTransform<'a> {
    HarnessBodyTransform {
        body: Cow::Borrowed(body),
        applied: false,
        policy,
    }
}

fn transformed<'a>(
    body: &'a [u8],
    value: serde_json::Value,
    applied: bool,
    policy: &'static HarnessProviderPolicySpec,
) -> Result<HarnessBodyTransform<'a>, HarnessProviderTransformError> {
    if !applied {
        return Ok(unchanged(body, Some(policy)));
    }
    Ok(HarnessBodyTransform {
        body: Cow::Owned(
            serde_json::to_vec(&value).expect("serializing an in-memory JSON value cannot fail"),
        ),
        applied: true,
        policy: Some(policy),
    })
}

fn reject_tool_alias_collisions(
    value: &serde_json::Value,
    aliases: &[HarnessToolAlias],
) -> Result<(), HarnessProviderTransformError> {
    let Some(tools) = value.get("tools").and_then(serde_json::Value::as_array) else {
        return Ok(());
    };
    let names = tools
        .iter()
        .filter_map(tool_declaration_name)
        .collect::<Vec<_>>();
    if aliases.iter().any(|alias| {
        names.iter().any(|name| *name == alias.canonical)
            && names.iter().any(|name| *name == alias.provider_native)
    }) {
        return Err(HarnessProviderTransformError::ToolAliasCollision);
    }
    Ok(())
}

fn tool_declaration_name(tool: &serde_json::Value) -> Option<&str> {
    tool.get("name")
        .or_else(|| tool.pointer("/function/name"))
        .and_then(serde_json::Value::as_str)
}

fn alias_request_tool_names(
    value: &mut serde_json::Value,
    aliases: &[HarnessToolAlias],
    reverse: bool,
) -> bool {
    let mut applied = false;
    if let Some(tools) = value
        .get_mut("tools")
        .and_then(serde_json::Value::as_array_mut)
    {
        for tool in tools {
            applied |= alias_name_at(tool, &["name"], aliases, reverse);
            applied |= alias_name_at(tool, &["function", "name"], aliases, reverse);
        }
    }
    if let Some(choice) = value.get_mut("tool_choice") {
        applied |= alias_name_at(choice, &["name"], aliases, reverse);
        applied |= alias_name_at(choice, &["function", "name"], aliases, reverse);
    }
    if let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    {
        for item in input {
            if matches!(
                item.get("type").and_then(serde_json::Value::as_str),
                Some("function_call" | "custom_tool_call")
            ) {
                applied |= alias_name_at(item, &["name"], aliases, reverse);
            }
            if let Some(tool_calls) = item
                .get_mut("tool_calls")
                .and_then(serde_json::Value::as_array_mut)
            {
                for call in tool_calls {
                    applied |= alias_name_at(call, &["function", "name"], aliases, reverse);
                }
            }
        }
    }
    applied
}

fn alias_response_tool_names(value: &mut serde_json::Value, aliases: &[HarnessToolAlias]) -> bool {
    let mut applied = alias_typed_call_name(value, aliases);
    for path in ["output", "item"] {
        if let Some(value) = value.get_mut(path) {
            applied |= alias_response_value(value, aliases);
        }
    }
    if let Some(response) = value.get_mut("response") {
        applied |= alias_response_value(response, aliases);
    }
    applied
}

fn alias_response_value(value: &mut serde_json::Value, aliases: &[HarnessToolAlias]) -> bool {
    match value {
        serde_json::Value::Array(values) => values.iter_mut().fold(false, |applied, value| {
            applied | alias_response_value(value, aliases)
        }),
        serde_json::Value::Object(_) => {
            let mut applied = alias_typed_call_name(value, aliases);
            if let Some(output) = value.get_mut("output") {
                applied |= alias_response_value(output, aliases);
            }
            if let Some(item) = value.get_mut("item") {
                applied |= alias_response_value(item, aliases);
            }
            applied
        }
        _ => false,
    }
}

fn alias_typed_call_name(value: &mut serde_json::Value, aliases: &[HarnessToolAlias]) -> bool {
    if !matches!(
        value.get("type").and_then(serde_json::Value::as_str),
        Some("function_call" | "custom_tool_call")
    ) {
        return false;
    }
    alias_name_at(value, &["name"], aliases, true)
}

fn alias_name_at(
    value: &mut serde_json::Value,
    path: &[&str],
    aliases: &[HarnessToolAlias],
    reverse: bool,
) -> bool {
    let mut target = value;
    for key in path {
        let Some(next) = target.get_mut(*key) else {
            return false;
        };
        target = next;
    }
    let Some(name) = target.as_str() else {
        return false;
    };
    let alias = alias_tool_name(name, aliases, reverse);
    if alias == name {
        return false;
    }
    *target = serde_json::Value::String(alias.into_owned());
    true
}

fn alias_tool_name<'a>(name: &'a str, aliases: &[HarnessToolAlias], reverse: bool) -> Cow<'a, str> {
    let alias = aliases.iter().find(|alias| {
        if reverse {
            alias.provider_native == name
        } else {
            alias.canonical == name
        }
    });
    alias.map_or(Cow::Borrowed(name), |alias| {
        Cow::Borrowed(if reverse {
            alias.canonical
        } else {
            alias.provider_native
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(body: &[u8]) -> Result<HarnessBodyTransform<'_>, HarnessProviderTransformError> {
        shape_harness_provider_request(
            EffectiveHarnessMode::Evaluated,
            ProviderId::Gemini,
            Some("gemini-3.1-pro-preview"),
            ProviderEndpoint::Responses,
            body,
        )
    }

    fn response(body: &[u8]) -> Result<HarnessBodyTransform<'_>, HarnessProviderTransformError> {
        postprocess_harness_provider_response(
            EffectiveHarnessMode::Evaluated,
            ProviderId::Gemini,
            Some("gemini-3.1-pro-preview"),
            ProviderEndpoint::Responses,
            body,
        )
    }

    #[test]
    fn catalog_is_versioned_provider_scoped_and_explicit() {
        assert_eq!(harness_provider_policy_catalog().len(), 2);
        let policy = harness_provider_policy_catalog()
            .iter()
            .find(|policy| policy.provider == ProviderId::Gemini)
            .unwrap();
        assert_eq!(
            policy.evaluation_id,
            "gemini-native-tool-alias-roundtrip-v1"
        );
        assert_eq!(policy.evaluation_version, 1);
        assert_eq!(policy.provider, ProviderId::Gemini);
        assert_eq!(policy.model, "*");
        assert!(!policy.minimal_instructions);
        assert!(!policy.native_anthropic_messages);
        assert_eq!(
            policy.response_policy,
            HarnessResponsePolicy::RestoreToolAliases
        );
        assert!(policy.continuity_tested);
        assert_eq!(policy.tool_aliases, GEMINI_TOOL_ALIASES);

        let anthropic = harness_provider_policy_catalog()
            .iter()
            .find(|policy| policy.provider == ProviderId::Anthropic)
            .unwrap();
        assert_eq!(
            anthropic.evaluation_id,
            "anthropic-native-messages-roundtrip-v1"
        );
        assert_eq!(anthropic.model, "*");
        assert!(anthropic.native_anthropic_messages);
        assert!(anthropic.tool_aliases.is_empty());
        assert_eq!(anthropic.response_policy, HarnessResponsePolicy::Disabled);
        assert!(anthropic.continuity_tested);
    }

    #[test]
    fn evaluated_anthropic_known_models_select_native_messages_only() {
        let policy = harness_provider_policy(
            EffectiveHarnessMode::Evaluated,
            ProviderId::Anthropic,
            Some("claude-sonnet-4-6"),
        )
        .unwrap();
        assert!(policy.native_anthropic_messages);
        assert!(
            harness_provider_policy(
                EffectiveHarnessMode::Native,
                ProviderId::Anthropic,
                Some("claude-sonnet-4-6"),
            )
            .is_none()
        );
        assert!(
            harness_provider_policy(
                EffectiveHarnessMode::Evaluated,
                ProviderId::Anthropic,
                Some("unknown-model"),
            )
            .is_none()
        );
    }

    #[test]
    fn disabled_and_unselected_provider_paths_preserve_exact_bytes() {
        let invalid = b"not json";
        for (mode, provider) in [
            (EffectiveHarnessMode::Native, ProviderId::Gemini),
            (EffectiveHarnessMode::Minimal, ProviderId::Gemini),
            (EffectiveHarnessMode::Evaluated, ProviderId::DeepSeek),
        ] {
            let shaped = shape_harness_provider_request(
                mode,
                provider,
                None,
                ProviderEndpoint::Responses,
                invalid,
            )
            .unwrap();
            assert!(matches!(shaped.body, Cow::Borrowed(_)));
            assert_eq!(shaped.body.as_ref(), invalid);
            assert!(!shaped.applied);
        }

        let unknown_model = shape_harness_provider_request(
            EffectiveHarnessMode::Evaluated,
            ProviderId::Gemini,
            Some("unknown-model"),
            ProviderEndpoint::Responses,
            invalid,
        )
        .unwrap();
        assert!(matches!(unknown_model.body, Cow::Borrowed(_)));
        assert!(!unknown_model.applied);
    }

    #[test]
    fn request_aliases_declarations_choice_and_continuation_history() {
        let input = json!({
            "model": "gemini-3.1-pro-preview",
            "tools": [{"type":"function", "name":"exec_command", "description":"kept", "parameters":{"type":"object"}}],
            "tool_choice": {"type":"function", "name":"exec_command"},
            "input": [
                {"type":"function_call", "call_id":"call_1", "name":"exec_command", "arguments":"{}"},
                {"role":"assistant", "tool_calls":[{"function":{"name":"exec_command", "arguments":"{}"}}]},
                {"type":"function_call_output", "call_id":"call_1", "output":"kept"}
            ],
            "unknown": {"name":"exec_command"}
        });
        let input = serde_json::to_vec(&input).unwrap();
        let shaped = request(&input).unwrap();
        assert!(shaped.applied);
        let output: serde_json::Value = serde_json::from_slice(&shaped.body).unwrap();
        assert_eq!(output["tools"][0]["name"], "run_shell_command");
        assert_eq!(output["tool_choice"]["name"], "run_shell_command");
        assert_eq!(output["input"][0]["name"], "run_shell_command");
        assert_eq!(
            output["input"][1]["tool_calls"][0]["function"]["name"],
            "run_shell_command"
        );
        assert_eq!(output["unknown"]["name"], "exec_command");

        let twice = request(&shaped.body).unwrap();
        assert!(!twice.applied);
        assert!(matches!(twice.body, Cow::Borrowed(_)));
    }

    #[test]
    fn request_rejects_ambiguous_alias_collisions() {
        let error = request(
            br#"{"tools":[{"type":"function","name":"exec_command"},{"type":"function","name":"run_shell_command"}]}"#,
        )
        .unwrap_err();
        assert_eq!(error, HarnessProviderTransformError::ToolAliasCollision);
        assert_eq!(error.code(), "tool-alias-collision");
    }

    #[test]
    fn buffered_response_restores_aliases_without_touching_untyped_names() {
        let input = json!({
            "id":"resp_1",
            "output":[
                {"type":"function_call", "call_id":"call_1", "name":"run_shell_command", "arguments":"{}"},
                {"type":"message", "name":"run_shell_command", "content":[]}
            ],
            "metadata":{"name":"run_shell_command"}
        });
        let input = serde_json::to_vec(&input).unwrap();
        let shaped = response(&input).unwrap();
        assert!(shaped.applied);
        let output: serde_json::Value = serde_json::from_slice(&shaped.body).unwrap();
        assert_eq!(output["output"][0]["name"], "exec_command");
        assert_eq!(output["output"][1]["name"], "run_shell_command");
        assert_eq!(output["metadata"]["name"], "run_shell_command");
        assert!(!response(&shaped.body).unwrap().applied);
    }

    #[test]
    fn stream_response_restores_alias_and_preserves_sse_framing() {
        let event = b"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"sequence_number\":2,\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"run_shell_command\",\"arguments\":\"\"}}\n\n";
        let shaped = postprocess_harness_provider_stream_event(
            EffectiveHarnessMode::Evaluated,
            ProviderId::Gemini,
            Some("gemini-3.1-pro-preview"),
            ProviderEndpoint::Responses,
            event,
        )
        .unwrap();
        assert!(shaped.applied);
        let output = std::str::from_utf8(&shaped.body).unwrap();
        assert!(output.starts_with("event: response.output_item.added\ndata: "));
        assert!(output.ends_with("\n\n"));
        assert!(output.contains("\"name\":\"exec_command\""));
        assert!(!output.contains("\"name\":\"run_shell_command\""));
    }

    #[test]
    fn alias_helpers_are_reversible_and_default_to_borrowed_noop() {
        let native = harness_provider_native_tool_name(
            EffectiveHarnessMode::Evaluated,
            ProviderId::Gemini,
            Some("gemini-3.1-pro-preview"),
            "exec_command",
        );
        assert_eq!(native, "run_shell_command");
        assert_eq!(
            harness_canonical_tool_name(
                EffectiveHarnessMode::Evaluated,
                ProviderId::Gemini,
                Some("gemini-3.1-pro-preview"),
                &native,
            ),
            "exec_command"
        );
        assert!(matches!(
            harness_provider_native_tool_name(
                EffectiveHarnessMode::Native,
                ProviderId::Gemini,
                None,
                "exec_command"
            ),
            Cow::Borrowed(_)
        ));
    }
}
