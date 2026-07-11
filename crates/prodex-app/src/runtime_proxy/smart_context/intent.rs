use super::*;

pub(super) fn runtime_smart_context_collect_intent_signals(
    value: &serde_json::Value,
) -> RuntimeSmartContextIntentSignals {
    let mut signals = RuntimeSmartContextIntentSignals {
        artifact_refs: runtime_smart_context_collect_rehydratable_artifact_refs(value),
        ..RuntimeSmartContextIntentSignals::default()
    };

    let artifact_ref_ids = signals
        .artifact_refs
        .iter()
        .map(|reference| reference.id.clone())
        .collect::<Vec<_>>();
    for id in artifact_ref_ids {
        signals.add_intent_text(&id);
    }

    runtime_smart_context_collect_user_intent_text(value, &mut signals);
    runtime_smart_context_collect_tool_intent_metadata(value, &mut signals);
    signals
}

impl RuntimeSmartContextIntentSignals {
    fn add_intent_text(&mut self, text: &str) {
        for term in prodex_context::extract_intent_terms_from_prompt(text) {
            if !self.add_intent_term(term) {
                break;
            }
        }
        for term in runtime_smart_context_error_code_terms_from_intent_text(text) {
            if !self.add_intent_term(term) {
                break;
            }
        }
        for kind in runtime_smart_context_command_kinds_from_intent_text(text) {
            self.semantic_terms.command_kinds.insert(kind.to_string());
        }
    }

    fn add_intent_term(&mut self, term: String) -> bool {
        if self.intent_terms.iter().any(|existing| existing == &term) {
            return true;
        }
        if self.intent_terms.len() >= prodex_context::MAX_EXTRACTED_INTENT_TERMS {
            return false;
        }
        self.add_semantic_term(&term);
        self.intent_terms.push(term);
        self.intent_terms.len() < prodex_context::MAX_EXTRACTED_INTENT_TERMS
    }

    fn add_command_kind_hint(&mut self, hint: prodex_context::CommandOutputKind) {
        self.command_kind_hints
            .insert(runtime_smart_context_command_kind_hint_label(hint).to_string());
    }

    fn add_semantic_term(&mut self, term: &str) {
        if runtime_smart_context_intent_term_is_error_code(term) {
            self.semantic_terms.error_codes.insert(term.to_string());
        } else if runtime_smart_context_intent_term_is_path(term) {
            self.semantic_terms.file_paths.insert(term.to_string());
        } else if runtime_smart_context_intent_term_is_symbol(term) {
            self.semantic_terms.test_symbols.insert(term.to_string());
        }
    }
}

pub(super) fn runtime_smart_context_collect_user_intent_text(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    if let Some(text) = value.get("prompt").and_then(serde_json::Value::as_str) {
        signals.add_intent_text(text);
    }
    match value.get("input") {
        Some(serde_json::Value::String(text)) => signals.add_intent_text(text),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_smart_context_collect_user_intent_text_from_input_item(item, signals);
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_collect_user_intent_text_from_input_item(
    item: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    if runtime_smart_context_value_is_static_context_item(item) {
        return;
    }
    let Some(object) = item.as_object() else {
        return;
    };
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if item_type.ends_with("_call_output") || item_type.ends_with("_call") {
        return;
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if role != "user" {
        return;
    }
    for key in ["content", "input_text", "text"] {
        if let Some(child) = object.get(key) {
            runtime_smart_context_collect_intent_text_parts(child, signals);
        }
    }
}

pub(super) fn runtime_smart_context_collect_tool_intent_metadata(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return;
    };
    for item in input {
        let Some(object) = item.as_object() else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        let metadata = runtime_smart_context_tool_item_metadata(object);
        if let Some(command) = metadata.command.as_deref() {
            signals.add_intent_text(command);
        }
        if let Some(kind_hint) = metadata.kind_hint {
            signals.add_command_kind_hint(kind_hint);
        }
    }
}

pub(super) fn runtime_smart_context_collect_intent_text_parts(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    match value {
        serde_json::Value::String(text) => signals.add_intent_text(text),
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_intent_text_parts(item, signals);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    runtime_smart_context_collect_intent_text_parts(item, signals);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_intent_term_is_error_code(term: &str) -> bool {
    if term.eq_ignore_ascii_case("error")
        || runtime_smart_context_numeric_error_term(term, "exit_code_")
        || runtime_smart_context_numeric_error_term(term, "status_code_")
    {
        return true;
    }
    let rest = term
        .strip_prefix('E')
        .or_else(|| term.strip_prefix("TS"))
        .or_else(|| term.strip_prefix('F'));
    rest.is_some_and(|value| value.len() >= 3 && value.chars().all(|ch| ch.is_ascii_digit()))
}

pub(super) fn runtime_smart_context_numeric_error_term(term: &str, prefix: &str) -> bool {
    term.strip_prefix(prefix)
        .is_some_and(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
}

pub(super) fn runtime_smart_context_intent_term_is_path(term: &str) -> bool {
    term.contains('/')
        || term.rsplit_once('.').is_some_and(|(_, extension)| {
            matches!(
                extension,
                "c" | "cc"
                    | "cpp"
                    | "css"
                    | "go"
                    | "h"
                    | "hpp"
                    | "html"
                    | "js"
                    | "jsx"
                    | "json"
                    | "md"
                    | "py"
                    | "rs"
                    | "toml"
                    | "ts"
                    | "tsx"
                    | "yaml"
                    | "yml"
            )
        })
}

pub(super) fn runtime_smart_context_intent_term_is_symbol(term: &str) -> bool {
    if term.contains("::") || term.contains('#') {
        return true;
    }
    if term.contains('.') && !runtime_smart_context_intent_term_is_path(term) {
        return term
            .split('.')
            .all(runtime_smart_context_intent_symbol_segment);
    }
    runtime_smart_context_intent_symbol_segment(term)
        && (term.starts_with("test_")
            || term.ends_with("_test")
            || term.contains('_')
            || runtime_smart_context_intent_symbol_has_camel_shape(term))
}

pub(super) fn runtime_smart_context_intent_symbol_segment(term: &str) -> bool {
    let mut chars = term.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

pub(super) fn runtime_smart_context_intent_symbol_has_camel_shape(term: &str) -> bool {
    let mut previous_lowercase = false;
    for ch in term.chars() {
        if previous_lowercase && ch.is_ascii_uppercase() {
            return true;
        }
        previous_lowercase = ch.is_ascii_lowercase();
    }
    false
}

pub(super) fn runtime_smart_context_error_code_terms_from_intent_text(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut terms = Vec::new();
    runtime_smart_context_push_numeric_error_terms(&lower, "exit code", "exit_code_", &mut terms);
    runtime_smart_context_push_numeric_error_terms(&lower, "exit_code", "exit_code_", &mut terms);
    runtime_smart_context_push_numeric_error_terms(
        &lower,
        "status code",
        "status_code_",
        &mut terms,
    );
    runtime_smart_context_push_numeric_error_terms(
        &lower,
        "status_code",
        "status_code_",
        &mut terms,
    );
    terms
}

pub(super) fn runtime_smart_context_push_numeric_error_terms(
    text: &str,
    marker: &str,
    prefix: &str,
    terms: &mut Vec<String>,
) {
    let mut remaining = text;
    while let Some(index) = remaining.find(marker) {
        let after_marker = &remaining[index + marker.len()..];
        let after_separator = after_marker
            .trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == ':' || ch == '=');
        let digits = after_separator
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if !digits.is_empty() {
            let term = format!("{prefix}{digits}");
            if !terms.iter().any(|existing| existing == &term) {
                terms.push(term);
            }
        }
        remaining = after_marker;
    }
}

pub(super) fn runtime_smart_context_command_kinds_from_intent_text(
    text: &str,
) -> Vec<&'static str> {
    let lower = text.to_ascii_lowercase();
    let mut kinds = Vec::new();
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(
            &lower,
            &["cargo test", "cargo nextest", "nextest run"],
        ),
        "cargo-test",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(
            &lower,
            &["cargo check", "cargo build", "cargo clippy"],
        ),
        "cargo-build",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(
            &lower,
            &["npm test", "pnpm test", "yarn test", "vitest", "jest"],
        ),
        "npm-test",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(&lower, &["git diff", "git show"]),
        "diff",
    );
    runtime_smart_context_push_command_kind_if(
        &mut kinds,
        runtime_smart_context_text_contains_any(&lower, &["pytest", "python -m pytest"]),
        "python",
    );
    kinds
}

pub(super) fn runtime_smart_context_text_contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

pub(super) fn runtime_smart_context_push_command_kind_if(
    kinds: &mut Vec<&'static str>,
    condition: bool,
    kind: &'static str,
) {
    if condition && !kinds.contains(&kind) {
        kinds.push(kind);
    }
}

pub(super) fn runtime_smart_context_command_kind_hint_label(
    hint: prodex_context::CommandOutputKind,
) -> &'static str {
    match hint {
        prodex_context::CommandOutputKind::Auto => "auto",
        prodex_context::CommandOutputKind::GitStatus => "git-status",
        prodex_context::CommandOutputKind::GitDiff => "git-diff",
        prodex_context::CommandOutputKind::RustDiagnostics => "rust-diagnostics",
        prodex_context::CommandOutputKind::Diagnostics => "diagnostics",
        prodex_context::CommandOutputKind::GitLog => "git-log",
        prodex_context::CommandOutputKind::Search => "search",
        prodex_context::CommandOutputKind::FileList => "file-list",
        prodex_context::CommandOutputKind::LogStream => "log-stream",
        prodex_context::CommandOutputKind::NoisySuccess => "noisy-success",
        prodex_context::CommandOutputKind::Plain => "plain",
    }
}
