use super::*;

pub(super) fn runtime_mem_conversation_elision_summary(
    kind: RuntimeMemConversationElisionKind,
    content: &str,
    command: Option<&str>,
    artifact_ref: Option<&str>,
) -> String {
    let mut parts = vec![format!(
        "mem ledger: kind={}; h={}; b={}; t~={}",
        kind.as_str(),
        runtime_mem_content_hash(content),
        content.len(),
        runtime_mem_approx_token_count(content)
    )];

    let objectives = runtime_mem_conversation_objective_facts(kind, content);
    runtime_mem_push_summary_facts(&mut parts, "objective", &objectives);

    let files = runtime_mem_conversation_file_facts(content);
    runtime_mem_push_summary_facts(&mut parts, "files", &files);

    let decisions = runtime_mem_conversation_line_facts(
        content,
        runtime_mem_conversation_line_has_decision_signal,
    );
    runtime_mem_push_summary_facts(&mut parts, "decisions", &decisions);

    let tests = runtime_mem_conversation_test_facts(content, command);
    runtime_mem_push_summary_facts(&mut parts, "tests", &tests);

    let failures =
        runtime_mem_conversation_line_facts(content, runtime_mem_summary_has_critical_signal);
    runtime_mem_push_summary_facts(&mut parts, "open_failures", &failures);

    let artifacts = runtime_mem_conversation_artifact_facts(content, artifact_ref);
    runtime_mem_push_summary_facts(&mut parts, "artifacts", &artifacts);

    runtime_mem_truncate_chars(
        &parts.join("; "),
        RUNTIME_MEM_CONVERSATION_LEDGER_SUMMARY_CHAR_LIMIT,
    )
}

fn runtime_mem_push_summary_facts(parts: &mut Vec<String>, label: &str, facts: &[String]) {
    if facts.is_empty() {
        return;
    }
    parts.push(format!("{label}=[{}]", facts.join(", ")));
}

fn runtime_mem_conversation_objective_facts(
    kind: RuntimeMemConversationElisionKind,
    content: &str,
) -> Vec<String> {
    let mut facts = Vec::new();
    for line in runtime_mem_conversation_scan_text(content).lines() {
        let Some(objective) = runtime_mem_conversation_objective_from_line(kind, line) else {
            continue;
        };
        runtime_mem_push_limited_unique_fact(&mut facts, objective);
        break;
    }
    facts
}

fn runtime_mem_conversation_objective_from_line(
    kind: RuntimeMemConversationElisionKind,
    line: &str,
) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    let explicit = [
        "objective:",
        "goal:",
        "task:",
        "request:",
        "user asked:",
        "implement ",
        "fix ",
        "add ",
        "update ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    if kind != RuntimeMemConversationElisionKind::User && !explicit {
        return None;
    }
    let normalized = line
        .strip_prefix("Objective:")
        .or_else(|| line.strip_prefix("objective:"))
        .or_else(|| line.strip_prefix("Goal:"))
        .or_else(|| line.strip_prefix("goal:"))
        .or_else(|| line.strip_prefix("Task:"))
        .or_else(|| line.strip_prefix("task:"))
        .or_else(|| line.strip_prefix("Request:"))
        .or_else(|| line.strip_prefix("request:"))
        .or_else(|| line.strip_prefix("User asked:"))
        .or_else(|| line.strip_prefix("user asked:"))
        .map(str::trim)
        .unwrap_or(line);
    Some(runtime_mem_truncate_chars(
        normalized,
        RUNTIME_MEM_CONVERSATION_LEDGER_OBJECTIVE_CHAR_LIMIT,
    ))
}

fn runtime_mem_conversation_file_facts(content: &str) -> Vec<String> {
    let scan = runtime_mem_conversation_scan_text(content);
    let mut facts = Vec::new();
    for raw in scan.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
    }) {
        let Some(path) = runtime_mem_conversation_normalize_path_fact(raw) else {
            continue;
        };
        runtime_mem_push_limited_unique_fact(&mut facts, path);
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_test_facts(content: &str, command: Option<&str>) -> Vec<String> {
    let mut facts = Vec::new();
    if let Some(command) = command
        .and_then(runtime_mem_conversation_normalize_command_fact)
        .filter(|command| runtime_mem_conversation_command_is_test(command))
    {
        runtime_mem_push_limited_unique_fact(&mut facts, command);
    }
    for line in runtime_mem_conversation_scan_text(content).lines() {
        let Some(command) = runtime_mem_conversation_command_from_line(line) else {
            continue;
        };
        if !runtime_mem_conversation_command_is_test(&command) {
            continue;
        }
        runtime_mem_push_limited_unique_fact(&mut facts, command);
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_command_is_test(command: &str) -> bool {
    let command = command.trim();
    command.starts_with("cargo test")
        || command.starts_with("cargo nextest")
        || command.starts_with("pytest")
        || command.starts_with("python -m pytest")
        || command.starts_with("python3 -m pytest")
        || command.starts_with("npm test")
        || command.starts_with("pnpm test")
        || command.starts_with("yarn test")
        || command.starts_with("go test")
        || command.starts_with("mvn test")
        || command.starts_with("gradle test")
        || command.starts_with("./gradlew test")
        || command.starts_with("just test")
        || command.contains(" cargo test ")
        || command.contains(" pytest ")
}

fn runtime_mem_conversation_artifact_facts(
    content: &str,
    artifact_ref: Option<&str>,
) -> Vec<String> {
    let mut facts = Vec::new();
    if let Some(artifact_ref) = artifact_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_mem_push_limited_unique_fact(&mut facts, artifact_ref.to_string());
    }
    let aliases = runtime_mem_artifact_aliases_from_text(content);
    for token in runtime_mem_artifact_ref_tokens(&runtime_mem_conversation_scan_text(content)) {
        let Some(artifact_ref) = runtime_mem_parse_artifact_ref_token(token, &aliases) else {
            continue;
        };
        runtime_mem_push_limited_unique_fact(&mut facts, artifact_ref);
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_line_facts(
    content: &str,
    predicate: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut facts = Vec::new();
    for line in runtime_mem_conversation_scan_text(content).lines() {
        let line = line.trim();
        if line.is_empty() || !predicate(line) {
            continue;
        }
        runtime_mem_push_limited_unique_fact(
            &mut facts,
            runtime_mem_truncate_chars(line, RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT),
        );
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_scan_text(content: &str) -> String {
    content
        .chars()
        .take(RUNTIME_MEM_CONVERSATION_ELISION_SCAN_CHAR_LIMIT)
        .collect()
}

fn runtime_mem_conversation_normalize_path_fact(raw: &str) -> Option<String> {
    let mut term = raw
        .trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches(|ch: char| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'))
        .trim_end_matches(['.', ',', ';', '!', '?']);
    while let Some((head, tail)) = term.rsplit_once(':') {
        if tail.is_empty() || !tail.chars().all(|ch| ch.is_ascii_digit()) {
            break;
        }
        term = head;
    }
    let term = term.trim_start_matches("./");
    if term.len() < 3 || !runtime_mem_prompt_term_is_path(term) {
        return None;
    }
    Some(runtime_mem_truncate_chars(
        term,
        RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT,
    ))
}

fn runtime_mem_conversation_command_from_line(line: &str) -> Option<String> {
    let line = line.trim();
    let command = line
        .strip_prefix('$')
        .or_else(|| line.strip_prefix('>'))
        .map(str::trim)
        .unwrap_or(line);
    if !runtime_mem_conversation_looks_like_command(command) {
        return None;
    }
    runtime_mem_conversation_normalize_command_fact(command)
}

fn runtime_mem_conversation_normalize_command_fact(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    Some(runtime_mem_truncate_chars(
        command,
        RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT,
    ))
}

fn runtime_mem_conversation_looks_like_command(command: &str) -> bool {
    [
        "./", "cargo ", "cargo-", "git ", "rg ", "grep ", "npm ", "pnpm ", "yarn ", "pytest",
        "python ", "python3 ", "node ", "prodex ", "codex ", "make ", "just ",
    ]
    .iter()
    .any(|prefix| command.starts_with(prefix))
}

fn runtime_mem_push_limited_unique_fact(facts: &mut Vec<String>, fact: String) {
    let fact = fact.trim();
    if fact.is_empty() || facts.iter().any(|existing| existing == fact) {
        return;
    }
    facts.push(fact.to_string());
}

fn runtime_mem_conversation_line_has_decision_signal(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    lower.contains("decision")
        || lower.contains("decided")
        || lower.contains("conclusion")
        || lower.contains("implemented")
        || lower.contains("changed")
        || lower.contains("fixed")
}
