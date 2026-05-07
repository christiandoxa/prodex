use super::*;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRepoStateFacts {
    pub(super) branch: Option<String>,
    pub(super) dirty_files: Option<BTreeSet<String>>,
    pub(super) recent_changed_files: Option<BTreeSet<String>>,
    pub(super) package_managers: Option<BTreeSet<String>>,
    pub(super) main_test_commands: Option<BTreeSet<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextRepoStateFactRelation {
    New,
    Repeated,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextRepoStateListKind {
    Dirty,
    RecentChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRepoStateLineSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRepoStateTextObservation {
    pub(super) facts: RuntimeSmartContextRepoStateFacts,
    pub(super) spans: Vec<RuntimeSmartContextRepoStateLineSpan>,
}

pub(super) fn runtime_smart_context_apply_repo_state_micro_cache(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
    request_id: u64,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    allow_rewrite: bool,
    stats: &mut RuntimeSmartContextTransformStats,
) -> bool {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return false;
    }
    let cache_before = state.repo_state_facts.clone();
    let mut cache_after = cache_before.clone();
    let mut rewritten = false;
    let tool_call_metadata = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .map(|input| runtime_smart_context_tool_call_metadata_by_call_id(input))
        .unwrap_or_default();

    for metadata in tool_call_metadata.values() {
        if let Some(command) = metadata.command.as_deref()
            && let Some(test_command) = runtime_smart_context_repo_state_test_command(command)
        {
            let mut facts = RuntimeSmartContextRepoStateFacts::default();
            runtime_smart_context_repo_state_insert_fact(
                &mut facts.main_test_commands,
                test_command,
            );
            cache_after.merge_from(&facts);
        }
    }

    let Some(object) = value.as_object_mut() else {
        if state.repo_state_facts != cache_after {
            state.repo_state_facts = cache_after;
        }
        return false;
    };
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(serde_json::Value::String(text)) = object.get_mut(key) {
            rewritten |= runtime_smart_context_apply_repo_state_micro_cache_to_text(
                text,
                None,
                &cache_before,
                &mut cache_after,
                &mut state.artifacts,
                request_id,
                allow_rewrite,
                stats,
            );
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        if state.repo_state_facts != cache_after {
            state.repo_state_facts = cache_after;
        }
        return rewritten;
    };

    for item in input {
        let Some(item_object) = item.as_object_mut() else {
            continue;
        };
        let item_type = item_object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let role = item_object
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        if runtime_smart_context_static_role_is_prompt_prefix(&role) {
            for field in ["content", "input_text"] {
                if let Some(serde_json::Value::String(text)) = item_object.get_mut(field) {
                    rewritten |= runtime_smart_context_apply_repo_state_micro_cache_to_text(
                        text,
                        None,
                        &cache_before,
                        &mut cache_after,
                        &mut state.artifacts,
                        request_id,
                        allow_rewrite,
                        stats,
                    );
                }
            }
            continue;
        }
        if !item_type.ends_with("_call_output") {
            continue;
        }
        let command = runtime_smart_context_tool_call_id(item_object)
            .and_then(|call_id| tool_call_metadata.get(call_id))
            .and_then(|metadata| metadata.command.as_deref())
            .map(str::to_string);
        for field in ["output", "content"] {
            if let Some(serde_json::Value::String(text)) = item_object.get_mut(field) {
                rewritten |= runtime_smart_context_apply_repo_state_micro_cache_to_text(
                    text,
                    command.as_deref(),
                    &cache_before,
                    &mut cache_after,
                    &mut state.artifacts,
                    request_id,
                    allow_rewrite,
                    stats,
                );
            }
        }
    }

    if state.repo_state_facts != cache_after {
        state.repo_state_facts = cache_after;
    }
    rewritten
}

#[allow(clippy::too_many_arguments)]
fn runtime_smart_context_apply_repo_state_micro_cache_to_text(
    text: &mut String,
    command: Option<&str>,
    cache_before: &RuntimeSmartContextRepoStateFacts,
    cache_after: &mut RuntimeSmartContextRepoStateFacts,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    allow_rewrite: bool,
    stats: &mut RuntimeSmartContextTransformStats,
) -> bool {
    let observation = runtime_smart_context_repo_state_text_observation(text, command);
    if observation.facts.is_empty() {
        return false;
    }
    let relation = observation.facts.relation_to(cache_before);
    cache_after.merge_from(&observation.facts);
    if !allow_rewrite
        || relation != RuntimeSmartContextRepoStateFactRelation::Repeated
        || observation.spans.is_empty()
    {
        return false;
    }

    let needs_artifact = text.len() >= SMART_CONTEXT_REPO_STATE_ARTIFACT_MIN_BYTES;
    let artifact_ref = needs_artifact.then(|| {
        let id = runtime_proxy_crate::smart_context_hash_text(text);
        runtime_smart_context_artifact_ref(&id)
    });
    let marker =
        runtime_smart_context_repo_state_repeat_marker(&observation.facts, artifact_ref.as_deref());
    let Some(candidate) =
        runtime_smart_context_repo_state_replaced_text(text, &observation.spans, &marker)
    else {
        return false;
    };
    if candidate.len() >= text.len()
        || prodex_context::critical_signal_self_check(text, &candidate).has_loss()
    {
        return false;
    }

    if needs_artifact {
        let existing_artifact = store.artifact_ref_for_exact_text(text.as_str());
        if existing_artifact.is_none() && store.insert_text(request_id, text.as_str()).is_none() {
            return false;
        }
        if existing_artifact.is_none() {
            stats.artifacts_stored = stats.artifacts_stored.saturating_add(1);
        }
    }

    *text = candidate;
    stats.repo_state_facts = stats.repo_state_facts.saturating_add(1);
    true
}

fn runtime_smart_context_repo_state_text_observation(
    text: &str,
    command: Option<&str>,
) -> RuntimeSmartContextRepoStateTextObservation {
    if text.contains(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX)
        || text.contains(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX_LEGACY)
    {
        return RuntimeSmartContextRepoStateTextObservation::default();
    }
    let command = command.unwrap_or_default().to_ascii_lowercase();
    let path_only_recent_mode =
        runtime_smart_context_repo_state_command_lists_recent_files(&command);
    let path_only_dirty_mode = runtime_smart_context_repo_state_command_lists_dirty_files(&command);

    let mut observation = RuntimeSmartContextRepoStateTextObservation::default();
    let mut active_list: Option<RuntimeSmartContextRepoStateListKind> = None;
    for (index, line) in text.split('\n').enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            active_list = None;
            continue;
        }

        let mut line_is_fact = false;
        if let Some((kind, inline)) = runtime_smart_context_repo_state_list_header(trimmed) {
            active_list = Some(kind);
            line_is_fact = true;
            for path in runtime_smart_context_repo_state_inline_paths(inline) {
                runtime_smart_context_repo_state_insert_path_by_kind(
                    &mut observation.facts,
                    kind,
                    path,
                );
            }
        }

        if let Some(branch) = runtime_smart_context_repo_state_branch(trimmed) {
            observation.facts.branch = Some(branch);
            line_is_fact = true;
        }

        let package_managers = runtime_smart_context_repo_state_package_managers_from_line(trimmed);
        if !package_managers.is_empty() {
            for package_manager in package_managers {
                runtime_smart_context_repo_state_insert_fact(
                    &mut observation.facts.package_managers,
                    package_manager,
                );
            }
            line_is_fact = true;
        }

        if let Some(test_command) = runtime_smart_context_repo_state_test_command(trimmed) {
            runtime_smart_context_repo_state_insert_fact(
                &mut observation.facts.main_test_commands,
                test_command,
            );
            line_is_fact = true;
        }

        if runtime_smart_context_repo_state_clean_status_line(trimmed) {
            observation
                .facts
                .dirty_files
                .get_or_insert_with(BTreeSet::new);
            line_is_fact = true;
        } else if let Some(path) = runtime_smart_context_repo_state_git_status_path(trimmed) {
            runtime_smart_context_repo_state_insert_fact(&mut observation.facts.dirty_files, path);
            line_is_fact = true;
        } else if let Some(kind) = active_list
            && let Some(path) = runtime_smart_context_repo_state_path_from_list_line(trimmed)
        {
            runtime_smart_context_repo_state_insert_path_by_kind(
                &mut observation.facts,
                kind,
                path,
            );
            line_is_fact = true;
        } else if path_only_dirty_mode
            && let Some(path) = runtime_smart_context_repo_state_path_from_list_line(trimmed)
        {
            runtime_smart_context_repo_state_insert_fact(&mut observation.facts.dirty_files, path);
            line_is_fact = true;
        } else if path_only_recent_mode
            && let Some(path) = runtime_smart_context_repo_state_path_from_list_line(trimmed)
        {
            runtime_smart_context_repo_state_insert_fact(
                &mut observation.facts.recent_changed_files,
                path,
            );
            line_is_fact = true;
        }

        if line_is_fact {
            observation
                .spans
                .push(RuntimeSmartContextRepoStateLineSpan {
                    start: index,
                    end: index + 1,
                });
        } else if active_list.is_some()
            && !runtime_smart_context_repo_state_list_continuation_line(trimmed)
        {
            active_list = None;
        }
    }
    observation.spans = runtime_smart_context_repo_state_merge_spans(observation.spans);
    observation
}

fn runtime_smart_context_repo_state_command_lists_recent_files(command: &str) -> bool {
    command.contains("git diff --name-only")
        || command.contains("git diff --name-status")
        || command.contains("git show --name-only")
        || command.contains("git show --name-status")
}

fn runtime_smart_context_repo_state_command_lists_dirty_files(command: &str) -> bool {
    command.contains("git status") && command.contains("--short")
}

fn runtime_smart_context_repo_state_list_header(
    line: &str,
) -> Option<(RuntimeSmartContextRepoStateListKind, &str)> {
    let lower = line.to_ascii_lowercase();
    let (head, tail) = line.split_once(':').unwrap_or((line, ""));
    let head_lower = head.trim().to_ascii_lowercase();
    if head_lower.contains("dirty files") || head_lower.contains("working tree files") {
        return Some((RuntimeSmartContextRepoStateListKind::Dirty, tail));
    }
    if head_lower.contains("recent changed files")
        || head_lower.contains("recent files")
        || head_lower.contains("changed files")
        || lower.starts_with("files changed:")
    {
        return Some((RuntimeSmartContextRepoStateListKind::RecentChanged, tail));
    }
    None
}

fn runtime_smart_context_repo_state_branch(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(branch) = trimmed.strip_prefix("On branch ") {
        return runtime_smart_context_repo_state_clean_branch(branch);
    }
    if let Some(branch) = trimmed
        .strip_prefix("Branch:")
        .or_else(|| trimmed.strip_prefix("branch:"))
        .or_else(|| trimmed.strip_prefix("Current branch:"))
        .or_else(|| trimmed.strip_prefix("current branch:"))
    {
        return runtime_smart_context_repo_state_clean_branch(branch);
    }
    if let Some(branch) = trimmed.strip_prefix("## ") {
        let branch = branch
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .split("...")
            .next()
            .unwrap_or_default();
        return runtime_smart_context_repo_state_clean_branch(branch);
    }
    None
}

fn runtime_smart_context_repo_state_clean_branch(value: &str) -> Option<String> {
    let branch = value
        .trim()
        .trim_matches(|ch: char| matches!(ch, '`' | '"' | '\'' | ',' | ';'));
    if branch.is_empty()
        || branch == "HEAD"
        || branch.len() > 160
        || branch.chars().any(char::is_control)
        || branch.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(branch.to_string())
}

fn runtime_smart_context_repo_state_clean_status_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("nothing to commit") && lower.contains("working tree clean")
}

fn runtime_smart_context_repo_state_git_status_path(line: &str) -> Option<String> {
    let trimmed = line.trim();
    for prefix in [
        "modified:",
        "new file:",
        "deleted:",
        "renamed:",
        "copied:",
        "both modified:",
        "both added:",
        "both deleted:",
        "added:",
        "updated:",
    ] {
        if let Some(path) = trimmed.strip_prefix(prefix) {
            return runtime_smart_context_repo_state_clean_path(path);
        }
    }

    if trimmed.starts_with("## ") || trimmed.len() < 4 {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let status_a = bytes[0] as char;
    let status_b = bytes[1] as char;
    if bytes[2] != b' '
        || !runtime_smart_context_repo_state_porcelain_status_char(status_a)
        || !runtime_smart_context_repo_state_porcelain_status_char(status_b)
        || (status_a == '!' && status_b == '!')
    {
        return None;
    }
    runtime_smart_context_repo_state_clean_path(&trimmed[3..])
}

fn runtime_smart_context_repo_state_porcelain_status_char(ch: char) -> bool {
    matches!(ch, ' ' | 'M' | 'A' | 'D' | 'R' | 'C' | 'U' | '?' | '!')
}

fn runtime_smart_context_repo_state_inline_paths(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter_map(runtime_smart_context_repo_state_path_from_list_line)
        .collect()
}

fn runtime_smart_context_repo_state_path_from_list_line(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches(['-', '*', '+']).trim();
    runtime_smart_context_repo_state_clean_path(trimmed)
}

fn runtime_smart_context_repo_state_clean_path(value: &str) -> Option<String> {
    let mut path = value
        .trim()
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '`' | '"' | '\'' | ',' | ';' | ':' | '[' | ']' | '(' | ')'
            )
        })
        .to_string();
    if let Some((_, renamed_to)) = path.rsplit_once(" -> ") {
        path = renamed_to.trim().to_string();
    }
    if let Some((status, rest)) = path.split_once('\t')
        && status.len() <= 2
        && status
            .chars()
            .all(runtime_smart_context_repo_state_porcelain_status_char)
    {
        path = rest.trim().to_string();
    }
    path = path
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .to_string();
    if path.is_empty()
        || path.len() > 240
        || path.chars().any(char::is_control)
        || path.chars().any(char::is_whitespace)
        || !runtime_smart_context_repo_state_path_like(&path)
    {
        return None;
    }
    Some(path)
}

fn runtime_smart_context_repo_state_path_like(path: &str) -> bool {
    if path.contains('/') {
        return true;
    }
    let lower = path.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "cargo.toml"
            | "cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lock"
            | "bun.lockb"
            | "pyproject.toml"
            | "uv.lock"
            | "requirements.txt"
            | "makefile"
    ) || lower.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension,
            "rs" | "toml"
                | "lock"
                | "json"
                | "yaml"
                | "yml"
                | "js"
                | "jsx"
                | "ts"
                | "tsx"
                | "py"
                | "md"
                | "go"
                | "java"
                | "kt"
                | "swift"
                | "c"
                | "cc"
                | "cpp"
                | "h"
                | "hpp"
                | "css"
                | "html"
        )
    })
}

fn runtime_smart_context_repo_state_list_continuation_line(line: &str) -> bool {
    line.starts_with('-')
        || line.starts_with('*')
        || runtime_smart_context_repo_state_path_from_list_line(line).is_some()
}

fn runtime_smart_context_repo_state_insert_path_by_kind(
    facts: &mut RuntimeSmartContextRepoStateFacts,
    kind: RuntimeSmartContextRepoStateListKind,
    path: String,
) {
    match kind {
        RuntimeSmartContextRepoStateListKind::Dirty => {
            runtime_smart_context_repo_state_insert_fact(&mut facts.dirty_files, path);
        }
        RuntimeSmartContextRepoStateListKind::RecentChanged => {
            runtime_smart_context_repo_state_insert_fact(&mut facts.recent_changed_files, path);
        }
    }
}

fn runtime_smart_context_repo_state_package_managers_from_line(line: &str) -> BTreeSet<String> {
    let lower = line.to_ascii_lowercase();
    let explicit = lower.contains("package manager")
        || lower.contains("package managers")
        || lower.contains("lockfile")
        || lower.contains("lock file");
    let mut managers = BTreeSet::new();
    if explicit || lower.contains("cargo.toml") || lower.contains("cargo.lock") {
        runtime_smart_context_repo_state_insert_package_if(
            &mut managers,
            lower.contains("cargo") || lower.contains("cargo.toml") || lower.contains("cargo.lock"),
            "cargo",
        );
    }
    if explicit || lower.contains("pnpm-lock.yaml") {
        runtime_smart_context_repo_state_insert_package_if(
            &mut managers,
            lower.contains("pnpm") || lower.contains("pnpm-lock.yaml"),
            "pnpm",
        );
    }
    if explicit || lower.contains("yarn.lock") {
        runtime_smart_context_repo_state_insert_package_if(
            &mut managers,
            lower.contains("yarn") || lower.contains("yarn.lock"),
            "yarn",
        );
    }
    if explicit || lower.contains("package.json") || lower.contains("package-lock.json") {
        runtime_smart_context_repo_state_insert_package_if(
            &mut managers,
            lower.contains("npm")
                || lower.contains("package.json")
                || lower.contains("package-lock.json"),
            "npm",
        );
    }
    if explicit || lower.contains("bun.lock") || lower.contains("bun.lockb") {
        runtime_smart_context_repo_state_insert_package_if(
            &mut managers,
            lower.contains("bun") || lower.contains("bun.lock"),
            "bun",
        );
    }
    if explicit || lower.contains("uv.lock") {
        runtime_smart_context_repo_state_insert_package_if(
            &mut managers,
            lower.contains("uv") || lower.contains("uv.lock"),
            "uv",
        );
    }
    managers
}

fn runtime_smart_context_repo_state_insert_package_if(
    managers: &mut BTreeSet<String>,
    condition: bool,
    value: &str,
) {
    if condition {
        managers.insert(value.to_string());
    }
}

fn runtime_smart_context_repo_state_test_command(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let after_label = trimmed
        .split_once(':')
        .filter(|(label, _)| {
            let label = label.to_ascii_lowercase();
            label.contains("test command")
                || label.contains("test commands")
                || label == "tests"
                || label == "main test"
        })
        .map(|(_, command)| command.trim())
        .unwrap_or(trimmed);
    let command = after_label
        .trim_start_matches(['-', '*', '+'])
        .trim()
        .trim_matches(|ch: char| matches!(ch, '`' | '"' | '\''));
    let lower = command.to_ascii_lowercase();
    if !runtime_smart_context_repo_state_text_contains_test_command(&lower)
        || command.len() > SMART_CONTEXT_REPO_STATE_TEST_COMMAND_MAX_CHARS
        || command.chars().any(char::is_control)
    {
        return None;
    }
    Some(command.to_string())
}

fn runtime_smart_context_repo_state_text_contains_test_command(lower: &str) -> bool {
    runtime_smart_context_text_contains_any(
        lower,
        &[
            "cargo test",
            "cargo nextest",
            "nextest run",
            "npm test",
            "npm run test",
            "pnpm test",
            "pnpm run test",
            "yarn test",
            "yarn run test",
            "bun test",
            "pytest",
            "python -m pytest",
            "go test",
        ],
    )
}

fn runtime_smart_context_repo_state_insert_fact(
    target: &mut Option<BTreeSet<String>>,
    value: String,
) {
    let values = target.get_or_insert_with(BTreeSet::new);
    if values.len() < SMART_CONTEXT_REPO_STATE_MAX_FACTS_PER_SET {
        values.insert(value);
    }
}

fn runtime_smart_context_repo_state_merge_spans(
    mut spans: Vec<RuntimeSmartContextRepoStateLineSpan>,
) -> Vec<RuntimeSmartContextRepoStateLineSpan> {
    spans.sort_by_key(|span| (span.start, span.end));
    let mut merged = Vec::<RuntimeSmartContextRepoStateLineSpan>::new();
    for span in spans {
        if span.end <= span.start {
            continue;
        }
        if let Some(last) = merged.last_mut()
            && span.start <= last.end
        {
            last.end = last.end.max(span.end);
            continue;
        }
        merged.push(span);
    }
    merged
}

fn runtime_smart_context_repo_state_replaced_text(
    text: &str,
    spans: &[RuntimeSmartContextRepoStateLineSpan],
    marker: &str,
) -> Option<String> {
    let spans = runtime_smart_context_repo_state_merge_spans(spans.to_vec());
    if spans.is_empty() {
        return None;
    }
    let lines = text.split('\n').collect::<Vec<_>>();
    let mut rendered = Vec::<String>::new();
    let mut line_index = 0usize;
    let mut span_index = 0usize;
    while line_index < lines.len() {
        if let Some(span) = spans.get(span_index)
            && line_index == span.start
        {
            rendered.push(marker.to_string());
            line_index = span.end.min(lines.len());
            span_index = span_index.saturating_add(1);
            continue;
        }
        rendered.push(lines[line_index].to_string());
        line_index = line_index.saturating_add(1);
    }
    Some(rendered.join("\n"))
}

fn runtime_smart_context_repo_state_repeat_marker(
    facts: &RuntimeSmartContextRepoStateFacts,
    artifact_ref: Option<&str>,
) -> String {
    let mut parts = vec![
        format!("{SMART_CONTEXT_REPO_STATE_MARKER_PREFIX}repeat"),
        format!(
            "h={}",
            runtime_smart_context_repo_state_facts_short_hash(facts)
        ),
    ];
    if let Some(branch) = facts.branch.as_deref() {
        parts.push(format!("branch={branch}"));
    }
    if let Some(dirty_files) = facts.dirty_files.as_ref() {
        parts.push(format!("dirty={}", dirty_files.len()));
    }
    if let Some(recent_changed_files) = facts.recent_changed_files.as_ref() {
        parts.push(format!("recent={}", recent_changed_files.len()));
    }
    if let Some(package_managers) = facts.package_managers.as_ref() {
        parts.push(format!(
            "pm={}",
            package_managers
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if let Some(commands) = facts.main_test_commands.as_ref() {
        parts.push(format!(
            "test='{}'",
            runtime_smart_context_repo_state_compact_commands(commands)
        ));
    }
    if let Some(artifact_ref) = artifact_ref {
        parts.push(format!("ref={artifact_ref}"));
    }
    parts.join(" ")
}

fn runtime_smart_context_repo_state_compact_commands(commands: &BTreeSet<String>) -> String {
    let joined = commands.iter().cloned().collect::<Vec<_>>().join("; ");
    if joined.len() <= SMART_CONTEXT_REPO_STATE_TEST_COMMAND_MAX_CHARS {
        joined
    } else {
        format!(
            "{} cmds h={}",
            commands.len(),
            runtime_smart_context_short_hash(&joined, SMART_CONTEXT_REPO_STATE_HASH_CHARS)
        )
    }
}

fn runtime_smart_context_repo_state_facts_short_hash(
    facts: &RuntimeSmartContextRepoStateFacts,
) -> String {
    runtime_smart_context_short_hash(&facts.canonical_text(), SMART_CONTEXT_REPO_STATE_HASH_CHARS)
}

fn runtime_smart_context_short_hash(text: &str, chars: usize) -> String {
    let hash = runtime_proxy_crate::smart_context_hash_text(text);
    hash.strip_prefix("sc:")
        .unwrap_or(hash.as_str())
        .chars()
        .take(chars)
        .collect()
}

impl RuntimeSmartContextRepoStateFacts {
    fn is_empty(&self) -> bool {
        self.branch.is_none()
            && self.dirty_files.is_none()
            && self.recent_changed_files.is_none()
            && self.package_managers.is_none()
            && self.main_test_commands.is_none()
    }

    fn merge_from(&mut self, other: &Self) {
        if other.branch.is_some() {
            self.branch = other.branch.clone();
        }
        if other.dirty_files.is_some() {
            self.dirty_files = other.dirty_files.clone();
        }
        if other.recent_changed_files.is_some() {
            self.recent_changed_files = other.recent_changed_files.clone();
        }
        if other.package_managers.is_some() {
            self.package_managers = other.package_managers.clone();
        }
        if other.main_test_commands.is_some() {
            self.main_test_commands = other.main_test_commands.clone();
        }
    }

    fn relation_to(&self, cached: &Self) -> RuntimeSmartContextRepoStateFactRelation {
        let mut has_new = false;
        if runtime_smart_context_repo_state_compare_fact(
            self.branch.as_ref(),
            cached.branch.as_ref(),
            &mut has_new,
        )
        .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.dirty_files.as_ref(),
                cached.dirty_files.as_ref(),
                &mut has_new,
            )
            .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.recent_changed_files.as_ref(),
                cached.recent_changed_files.as_ref(),
                &mut has_new,
            )
            .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.package_managers.as_ref(),
                cached.package_managers.as_ref(),
                &mut has_new,
            )
            .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.main_test_commands.as_ref(),
                cached.main_test_commands.as_ref(),
                &mut has_new,
            )
            .is_none()
        {
            return RuntimeSmartContextRepoStateFactRelation::Changed;
        }
        if has_new {
            RuntimeSmartContextRepoStateFactRelation::New
        } else {
            RuntimeSmartContextRepoStateFactRelation::Repeated
        }
    }

    fn canonical_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some(branch) = self.branch.as_ref() {
            lines.push(format!("branch={branch}"));
        }
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "dirty",
            self.dirty_files.as_ref(),
        );
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "recent",
            self.recent_changed_files.as_ref(),
        );
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "pm",
            self.package_managers.as_ref(),
        );
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "test",
            self.main_test_commands.as_ref(),
        );
        lines.join("\n")
    }
}

fn runtime_smart_context_repo_state_compare_fact<T: Eq>(
    current: Option<&T>,
    cached: Option<&T>,
    has_new: &mut bool,
) -> Option<RuntimeSmartContextRepoStateFactRelation> {
    let Some(current) = current else {
        return Some(RuntimeSmartContextRepoStateFactRelation::Repeated);
    };
    let Some(cached) = cached else {
        *has_new = true;
        return Some(RuntimeSmartContextRepoStateFactRelation::New);
    };
    (current == cached).then_some(RuntimeSmartContextRepoStateFactRelation::Repeated)
}

fn runtime_smart_context_repo_state_push_canonical_set(
    lines: &mut Vec<String>,
    label: &str,
    values: Option<&BTreeSet<String>>,
) {
    if let Some(values) = values {
        lines.push(format!("{label}:"));
        lines.extend(values.iter().map(|value| format!("- {value}")));
    }
}
