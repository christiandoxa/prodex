use crate::{ALL_PROVIDER_ENDPOINTS, ProviderEndpoint};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt, str::FromStr};

pub const MINIMAL_HARNESS_INSTRUCTIONS: &str = "[Prodex harness: minimal/v1]\nAct as a focused coding agent. Inspect the relevant code before editing, use\navailable tools to make concrete progress, keep changes minimal, and run the\nsmallest relevant verification before finishing.";
const MINIMAL_HARNESS_MARKER: &str = "[Prodex harness: minimal/v1]";
const RESPONSES_ROUTE: &[ProviderEndpoint] = &[ProviderEndpoint::Responses];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarnessMode {
    #[default]
    Auto,
    Native,
    Minimal,
}

impl HarnessMode {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::Minimal => "minimal",
        }
    }
}

impl fmt::Display for HarnessMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseHarnessModeError(String);

impl fmt::Display for ParseHarnessModeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown harness mode `{}`", self.0)
    }
}

impl std::error::Error for ParseHarnessModeError {}

impl FromStr for HarnessMode {
    type Err = ParseHarnessModeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "native" => Ok(Self::Native),
            "minimal" => Ok(Self::Minimal),
            _ => Err(ParseHarnessModeError(value.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffectiveHarnessMode {
    Native,
    Minimal,
}

impl EffectiveHarnessMode {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Minimal => "minimal",
        }
    }
}

impl fmt::Display for EffectiveHarnessMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessResolutionSource {
    Cli,
    Config,
    Default,
}

impl HarnessResolutionSource {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Config => "config",
            Self::Default => "default",
        }
    }
}

impl fmt::Display for HarnessResolutionSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedHarnessMode {
    pub requested: HarnessMode,
    pub effective: EffectiveHarnessMode,
    pub source: HarnessResolutionSource,
    pub reason: String,
}

impl ResolvedHarnessMode {
    pub const fn reason_code(&self) -> &'static str {
        match self.requested {
            HarnessMode::Auto => "v1-conservative-auto-default",
            HarnessMode::Native | HarnessMode::Minimal => "explicit-selection",
        }
    }
}

pub fn resolve_harness_mode(
    cli: Option<HarnessMode>,
    config: Option<HarnessMode>,
) -> ResolvedHarnessMode {
    let (requested, source, selection_reason) = if let Some(mode) = cli {
        (mode, HarnessResolutionSource::Cli, "explicit CLI selection")
    } else if let Some(mode) = config {
        (
            mode,
            HarnessResolutionSource::Config,
            "explicit config selection",
        )
    } else {
        (
            HarnessMode::Auto,
            HarnessResolutionSource::Default,
            "v1 conservative auto default",
        )
    };
    let effective = match requested {
        HarnessMode::Auto | HarnessMode::Native => EffectiveHarnessMode::Native,
        HarnessMode::Minimal => EffectiveHarnessMode::Minimal,
    };
    let reason = if requested == HarnessMode::Auto && source != HarnessResolutionSource::Default {
        format!("{selection_reason}; v1 conservative auto default")
    } else {
        selection_reason.to_string()
    };
    ResolvedHarnessMode {
        requested,
        effective,
        source,
        reason,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HarnessModeSpec {
    pub mode: HarnessMode,
    pub id: &'static str,
    pub display_label: &'static str,
    pub description: &'static str,
    pub selectable: bool,
    pub default_effective_mode: EffectiveHarnessMode,
    pub supported_canonical_request_routes: &'static [ProviderEndpoint],
    pub request_shaping: bool,
    pub response_shaping: bool,
    pub stream_shaping: bool,
}

pub const HARNESS_MODE_CATALOG: &[HarnessModeSpec] = &[
    HarnessModeSpec {
        mode: HarnessMode::Auto,
        id: "auto",
        display_label: "Auto",
        description: "Conservative automatic selection; resolves to Native in v1.",
        selectable: true,
        default_effective_mode: EffectiveHarnessMode::Native,
        supported_canonical_request_routes: ALL_PROVIDER_ENDPOINTS,
        request_shaping: false,
        response_shaping: false,
        stream_shaping: false,
    },
    HarnessModeSpec {
        mode: HarnessMode::Native,
        id: "native",
        display_label: "Native",
        description: "Preserves existing bridge behavior without harness shaping.",
        selectable: true,
        default_effective_mode: EffectiveHarnessMode::Native,
        supported_canonical_request_routes: ALL_PROVIDER_ENDPOINTS,
        request_shaping: false,
        response_shaping: false,
        stream_shaping: false,
    },
    HarnessModeSpec {
        mode: HarnessMode::Minimal,
        id: "minimal",
        display_label: "Minimal",
        description: "Prepends the minimal/v1 instruction block to canonical Responses requests.",
        selectable: true,
        default_effective_mode: EffectiveHarnessMode::Minimal,
        supported_canonical_request_routes: RESPONSES_ROUTE,
        request_shaping: true,
        response_shaping: false,
        stream_shaping: false,
    },
];

pub const fn harness_mode_catalog() -> &'static [HarnessModeSpec] {
    HARNESS_MODE_CATALOG
}

#[derive(Debug, PartialEq, Eq)]
pub struct HarnessShapedRequest<'a> {
    pub body: Cow<'a, [u8]>,
    pub headers: Cow<'a, [(String, String)]>,
    pub applied: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarnessRequestShapeError {
    InvalidJson,
    RequestMustBeObject,
    StructuredInstructions,
}

impl HarnessRequestShapeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid-json",
            Self::RequestMustBeObject => "request-must-be-object",
            Self::StructuredInstructions => "structured-instructions",
        }
    }
}

impl fmt::Display for HarnessRequestShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJson => "harness request body is not valid JSON",
            Self::RequestMustBeObject => "harness request body must be a JSON object",
            Self::StructuredInstructions => {
                "minimal harness does not support structured instructions"
            }
        })
    }
}

impl std::error::Error for HarnessRequestShapeError {}

pub fn shape_harness_request<'a>(
    mode: EffectiveHarnessMode,
    endpoint: ProviderEndpoint,
    body: &'a [u8],
    headers: &'a [(String, String)],
) -> Result<HarnessShapedRequest<'a>, HarnessRequestShapeError> {
    if mode == EffectiveHarnessMode::Native || endpoint != ProviderEndpoint::Responses {
        return Ok(HarnessShapedRequest {
            body: Cow::Borrowed(body),
            headers: Cow::Borrowed(headers),
            applied: false,
        });
    }

    let mut value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| HarnessRequestShapeError::InvalidJson)?;
    let object = value
        .as_object_mut()
        .ok_or(HarnessRequestShapeError::RequestMustBeObject)?;
    let instructions = match object.get("instructions") {
        None | Some(serde_json::Value::Null) => MINIMAL_HARNESS_INSTRUCTIONS.to_string(),
        Some(serde_json::Value::String(existing)) if existing.contains(MINIMAL_HARNESS_MARKER) => {
            return Ok(HarnessShapedRequest {
                body: Cow::Borrowed(body),
                headers: Cow::Borrowed(headers),
                applied: false,
            });
        }
        Some(serde_json::Value::String(existing)) => {
            format!("{MINIMAL_HARNESS_INSTRUCTIONS}\n\n{existing}")
        }
        Some(_) => return Err(HarnessRequestShapeError::StructuredInstructions),
    };
    object.insert(
        "instructions".to_string(),
        serde_json::Value::String(instructions),
    );
    Ok(HarnessShapedRequest {
        body: Cow::Owned(
            serde_json::to_vec(&value).expect("serializing an in-memory JSON value cannot fail"),
        ),
        headers: Cow::Borrowed(headers),
        applied: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shape(body: &[u8]) -> Result<HarnessShapedRequest<'_>, HarnessRequestShapeError> {
        static HEADERS: std::sync::LazyLock<Vec<(String, String)>> =
            std::sync::LazyLock::new(|| vec![("x-test".into(), "kept".into())]);
        shape_harness_request(
            EffectiveHarnessMode::Minimal,
            ProviderEndpoint::Responses,
            body,
            &HEADERS,
        )
    }

    #[test]
    fn modes_parse_display_serialize_and_reject_unknown_values() {
        for (text, mode) in [
            ("auto", HarnessMode::Auto),
            ("native", HarnessMode::Native),
            ("minimal", HarnessMode::Minimal),
        ] {
            assert_eq!(text.parse::<HarnessMode>().unwrap(), mode);
            assert_eq!(mode.to_string(), text);
            assert_eq!(
                serde_json::to_string(&mode).unwrap(),
                format!(r#""{text}""#)
            );
            assert_eq!(
                serde_json::from_str::<HarnessMode>(&format!(r#""{text}""#)).unwrap(),
                mode
            );
        }
        assert!("unknown".parse::<HarnessMode>().is_err());
        assert!(serde_json::from_str::<HarnessMode>(r#""unknown""#).is_err());
    }

    #[test]
    fn resolution_is_conservative_and_cli_wins() {
        let native = resolve_harness_mode(Some(HarnessMode::Native), None);
        assert_eq!(native.effective, EffectiveHarnessMode::Native);
        assert_eq!(native.source, HarnessResolutionSource::Cli);
        let minimal = resolve_harness_mode(None, Some(HarnessMode::Minimal));
        assert_eq!(minimal.effective, EffectiveHarnessMode::Minimal);
        assert_eq!(minimal.source, HarnessResolutionSource::Config);
        let auto = resolve_harness_mode(None, None);
        assert_eq!(auto.requested, HarnessMode::Auto);
        assert_eq!(auto.effective, EffectiveHarnessMode::Native);
        assert_eq!(auto.reason_code(), "v1-conservative-auto-default");
        assert_eq!(HarnessResolutionSource::Cli.id(), "cli");
        assert_eq!(HarnessResolutionSource::Config.to_string(), "config");
        assert_eq!(HarnessResolutionSource::Default.id(), "default");
        assert_eq!(
            resolve_harness_mode(Some(HarnessMode::Native), Some(HarnessMode::Minimal)).requested,
            HarnessMode::Native
        );
    }

    #[test]
    fn native_preserves_exact_bytes_and_headers_without_ownership() {
        let body = br#"{ "input": "exact bytes" }"#;
        let headers = vec![
            ("X-Test".into(), "first".into()),
            ("X-Test".into(), "second".into()),
        ];
        let shaped = shape_harness_request(
            EffectiveHarnessMode::Native,
            ProviderEndpoint::Responses,
            body,
            &headers,
        )
        .unwrap();
        assert!(matches!(shaped.body, Cow::Borrowed(_)));
        assert!(matches!(shaped.headers, Cow::Borrowed(_)));
        assert_eq!(shaped.body.as_ref(), body);
        assert_eq!(shaped.headers.as_ref(), headers.as_slice());
        assert!(!shaped.applied);
    }

    #[test]
    fn minimal_adds_and_prepends_instructions_idempotently() {
        let absent = shape(br#"{"model":"m","input":"hi"}"#).unwrap();
        let absent_value: serde_json::Value = serde_json::from_slice(&absent.body).unwrap();
        assert_eq!(absent_value["instructions"], MINIMAL_HARNESS_INSTRUCTIONS);

        let null = shape(br#"{"instructions":null,"input":"hi"}"#).unwrap();
        let null_value: serde_json::Value = serde_json::from_slice(&null.body).unwrap();
        assert_eq!(null_value["instructions"], MINIMAL_HARNESS_INSTRUCTIONS);

        let existing = shape(br#"{"instructions":"Keep this.","input":"hi"}"#).unwrap();
        let existing_value: serde_json::Value = serde_json::from_slice(&existing.body).unwrap();
        assert_eq!(
            existing_value["instructions"],
            format!("{MINIMAL_HARNESS_INSTRUCTIONS}\n\nKeep this.")
        );

        let twice = shape(&existing.body).unwrap();
        assert!(matches!(twice.body, Cow::Borrowed(_)));
        assert_eq!(twice.body, existing.body);
        assert!(!twice.applied);
    }

    #[test]
    fn minimal_preserves_request_semantics() {
        let input = json!({
            "model": "m",
            "input": [{"role": "user", "content": "hi"}],
            "previous_response_id": "response_1",
            "tools": [{"type": "function", "name": "run", "parameters": {"type": "object", "properties": {"x": {"type": "string"}}}}],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "reasoning": {"effort": "high"},
            "stream": true,
            "metadata": {"unknown": 7},
            "future_field": {"nested": [1, 2, 3]}
        });
        let input_bytes = serde_json::to_vec(&input).unwrap();
        let shaped = shape(&input_bytes).unwrap();
        let mut output: serde_json::Value = serde_json::from_slice(&shaped.body).unwrap();
        output.as_object_mut().unwrap().remove("instructions");
        assert_eq!(output, input);
    }

    #[test]
    fn minimal_rejects_structured_instructions() {
        let error = shape(br#"{"instructions":[{"text":"no"}]}"#).unwrap_err();
        assert_eq!(error, HarnessRequestShapeError::StructuredInstructions);
        assert_eq!(error.code(), "structured-instructions");
        assert!(!error.to_string().contains("text"));
        assert_eq!(HarnessRequestShapeError::InvalidJson.code(), "invalid-json");
        assert_eq!(
            HarnessRequestShapeError::RequestMustBeObject.code(),
            "request-must-be-object"
        );
    }

    #[test]
    fn minimal_does_not_shape_non_responses_routes() {
        let body = br#"{"instructions":{"structured":true}}"#;
        let headers = Vec::new();
        let shaped = shape_harness_request(
            EffectiveHarnessMode::Minimal,
            ProviderEndpoint::ResponsesCompact,
            body,
            &headers,
        )
        .unwrap();
        assert!(matches!(shaped.body, Cow::Borrowed(_)));
        assert_eq!(shaped.body.as_ref(), body);
        assert!(!shaped.applied);
    }

    #[test]
    fn catalog_is_stable_complete_and_has_no_response_shaping() {
        assert_eq!(
            harness_mode_catalog()
                .iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            ["auto", "native", "minimal"]
        );
        assert!(harness_mode_catalog().iter().all(|spec| spec.selectable));
        assert!(
            harness_mode_catalog()
                .iter()
                .all(|spec| !spec.response_shaping && !spec.stream_shaping)
        );
    }
}
