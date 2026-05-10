use super::*;
use std::path::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct RuntimeMemRecallIntentScore {
    path_score: usize,
    symbol_score: usize,
}

impl RuntimeMemRecallIntentScore {
    pub(super) fn total(self) -> usize {
        self.path_score.saturating_add(self.symbol_score)
    }

    pub(super) fn is_match(self) -> bool {
        self.total() > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeMemPathIntentMatch {
    None,
    FileName,
    Overlap,
    Exact,
}

impl RuntimeMemPathIntentMatch {
    fn score(self) -> usize {
        match self {
            RuntimeMemPathIntentMatch::None => 0,
            RuntimeMemPathIntentMatch::FileName => 2,
            RuntimeMemPathIntentMatch::Overlap => 5,
            RuntimeMemPathIntentMatch::Exact => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeMemSymbolIntentMatch {
    None,
    Qualified,
    Exact,
}

impl RuntimeMemSymbolIntentMatch {
    fn score(self) -> usize {
        match self {
            RuntimeMemSymbolIntentMatch::None => 0,
            RuntimeMemSymbolIntentMatch::Qualified => 4,
            RuntimeMemSymbolIntentMatch::Exact => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct RuntimeMemPreparedRecallIntent {
    paths: Vec<PathBuf>,
    symbols: Vec<String>,
}

impl RuntimeMemPreparedRecallIntent {
    pub(super) fn from_intent(intent: &RuntimeMemRecallIntent) -> Self {
        let mut prepared = Self::default();
        for path in &intent.paths {
            runtime_mem_push_unique_path(&mut prepared.paths, path.clone());
        }
        for symbol in &intent.symbols {
            runtime_mem_push_unique_symbol(&mut prepared.symbols, symbol);
        }
        if let Some(prompt) = intent.prompt.as_deref() {
            for path in runtime_mem_prompt_intent_paths(prompt) {
                runtime_mem_push_unique_path(&mut prepared.paths, path);
            }
            for symbol in runtime_mem_prompt_intent_symbols(prompt) {
                runtime_mem_push_unique_symbol(&mut prepared.symbols, &symbol);
            }
        }
        prepared
    }

    pub(super) fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.symbols.is_empty()
    }
}

pub(super) fn runtime_mem_capsule_intent_score(
    capsule: &RuntimeMemRecallCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
    intent: &RuntimeMemPreparedRecallIntent,
) -> RuntimeMemRecallIntentScore {
    let path_score = intent
        .paths
        .iter()
        .map(|intent_path| runtime_mem_capsule_path_intent_match(capsule, context, intent_path))
        .map(RuntimeMemPathIntentMatch::score)
        .sum::<usize>();
    let symbol_score = intent
        .symbols
        .iter()
        .map(|intent_symbol| {
            capsule
                .symbols
                .iter()
                .map(|capsule_symbol| {
                    runtime_mem_symbols_match(capsule_symbol.as_str(), intent_symbol.as_str())
                })
                .max()
                .unwrap_or(RuntimeMemSymbolIntentMatch::None)
        })
        .map(RuntimeMemSymbolIntentMatch::score)
        .sum::<usize>();

    RuntimeMemRecallIntentScore {
        path_score,
        symbol_score,
    }
}

pub(super) fn runtime_mem_recall_diet_capsule_order(
    left: &(
        RuntimeMemRecallCapsuleMetadata,
        RuntimeMemCapsulePriority,
        RuntimeMemRecallIntentScore,
    ),
    right: &(
        RuntimeMemRecallCapsuleMetadata,
        RuntimeMemCapsulePriority,
        RuntimeMemRecallIntentScore,
    ),
) -> Ordering {
    runtime_mem_recall_diet_rank(left.1, left.2)
        .cmp(&runtime_mem_recall_diet_rank(right.1, right.2))
        .then_with(|| right.2.total().cmp(&left.2.total()))
        .then_with(|| {
            right
                .0
                .capsule
                .updated_at_seconds
                .unwrap_or(i64::MIN)
                .cmp(&left.0.capsule.updated_at_seconds.unwrap_or(i64::MIN))
        })
        .then_with(|| {
            runtime_mem_relevance_order(right.0.capsule.relevance, left.0.capsule.relevance)
        })
        .then_with(|| left.0.capsule.token_cost.cmp(&right.0.capsule.token_cost))
        .then_with(|| left.0.capsule.id.cmp(&right.0.capsule.id))
}

fn runtime_mem_recall_diet_rank(
    priority: RuntimeMemCapsulePriority,
    intent_score: RuntimeMemRecallIntentScore,
) -> u8 {
    match (priority, intent_score.is_match()) {
        (RuntimeMemCapsulePriority::Required, _) => 0,
        (RuntimeMemCapsulePriority::ProjectLocal, true) => 1,
        (RuntimeMemCapsulePriority::ProjectLocal, false) => 2,
        (RuntimeMemCapsulePriority::Recent, true) => 3,
        (RuntimeMemCapsulePriority::Optional, true) => 4,
        (RuntimeMemCapsulePriority::Recent, false) => 5,
        (RuntimeMemCapsulePriority::Optional, false) => 6,
    }
}

fn runtime_mem_capsule_path_intent_match(
    capsule: &RuntimeMemRecallCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
    intent_path: &Path,
) -> RuntimeMemPathIntentMatch {
    let intent_file_name = runtime_mem_single_component_file_name(intent_path);
    let intent_path =
        runtime_mem_intent_path_for_matching(intent_path, context.project_root.as_deref());
    capsule
        .capsule
        .project_path
        .iter()
        .chain(capsule.paths.iter())
        .map(|path| {
            let capsule_path =
                runtime_mem_intent_path_for_matching(path, context.project_root.as_deref());
            if capsule_path == intent_path {
                RuntimeMemPathIntentMatch::Exact
            } else if runtime_mem_paths_overlap(&capsule_path, &intent_path) {
                RuntimeMemPathIntentMatch::Overlap
            } else if intent_file_name.is_some_and(|intent_file_name| {
                path.file_name().and_then(|name| name.to_str()) == Some(intent_file_name)
            }) {
                RuntimeMemPathIntentMatch::FileName
            } else {
                RuntimeMemPathIntentMatch::None
            }
        })
        .max()
        .unwrap_or(RuntimeMemPathIntentMatch::None)
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

fn runtime_mem_single_component_file_name(path: &Path) -> Option<&str> {
    let mut components = path.components();
    let Component::Normal(component) = components.next()? else {
        return None;
    };
    if components.next().is_some() {
        return None;
    }
    component.to_str()
}

fn runtime_mem_symbols_match(
    capsule_symbol: &str,
    intent_symbol: &str,
) -> RuntimeMemSymbolIntentMatch {
    let capsule_symbol = runtime_mem_normalized_symbol(capsule_symbol);
    let intent_symbol = runtime_mem_normalized_symbol(intent_symbol);
    if capsule_symbol.is_empty() || intent_symbol.is_empty() {
        return RuntimeMemSymbolIntentMatch::None;
    }
    if capsule_symbol == intent_symbol {
        return RuntimeMemSymbolIntentMatch::Exact;
    }
    if capsule_symbol.ends_with(&format!("::{intent_symbol}"))
        || capsule_symbol.ends_with(&format!(".{intent_symbol}"))
        || intent_symbol.ends_with(&format!("::{capsule_symbol}"))
        || intent_symbol.ends_with(&format!(".{capsule_symbol}"))
    {
        return RuntimeMemSymbolIntentMatch::Qualified;
    }
    RuntimeMemSymbolIntentMatch::None
}

fn runtime_mem_prompt_intent_paths(prompt: &str) -> Vec<PathBuf> {
    runtime_mem_recall_prompt_terms(prompt)
        .into_iter()
        .filter(|term| runtime_mem_prompt_term_is_path(term))
        .take(RUNTIME_MEM_RECALL_PROMPT_MAX_TERMS)
        .map(PathBuf::from)
        .collect()
}

fn runtime_mem_prompt_intent_symbols(prompt: &str) -> Vec<String> {
    runtime_mem_recall_prompt_terms(prompt)
        .into_iter()
        .filter(|term| runtime_mem_prompt_term_is_symbol(term))
        .take(RUNTIME_MEM_RECALL_PROMPT_MAX_TERMS)
        .collect()
}

fn runtime_mem_recall_prompt_terms(prompt: &str) -> Vec<String> {
    let prompt = prompt
        .chars()
        .take(RUNTIME_MEM_RECALL_PROMPT_SCAN_CHAR_LIMIT)
        .collect::<String>();
    let mut terms = Vec::new();
    for raw in prompt.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
    }) {
        let Some(term) = runtime_mem_normalize_prompt_term(raw) else {
            continue;
        };
        if !terms.iter().any(|existing| existing == &term) {
            terms.push(term);
        }
    }
    terms
}

fn runtime_mem_normalize_prompt_term(raw: &str) -> Option<String> {
    let term = raw
        .trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(['.', ':', '!', '?', ',']);
    let term = runtime_mem_strip_path_locator_suffix(term);
    (!term.is_empty()).then(|| term.to_string())
}

fn runtime_mem_strip_path_locator_suffix(term: &str) -> String {
    let mut trimmed = term.trim_end_matches([')', ']', '}', '"', '\'', '`', ',', ';']);
    while let Some((prefix, suffix)) = trimmed.rsplit_once(':') {
        if prefix.contains("::")
            || prefix.is_empty()
            || !suffix.chars().all(|ch| ch.is_ascii_digit())
        {
            break;
        }
        trimmed = prefix;
    }
    if let Some((prefix, suffix)) = trimmed.rsplit_once("#L")
        && !prefix.is_empty()
        && suffix.chars().all(|ch| ch.is_ascii_digit() || ch == '-')
    {
        return prefix.to_string();
    }
    trimmed.to_string()
}

pub(crate) fn runtime_mem_prompt_term_is_path(term: &str) -> bool {
    term.contains('/')
        || term.contains('\\')
        || Path::new(term)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(runtime_mem_prompt_term_extension_is_path_like)
}

fn runtime_mem_prompt_term_extension_is_path_like(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "c" | "cc"
            | "cpp"
            | "css"
            | "go"
            | "h"
            | "hpp"
            | "html"
            | "java"
            | "js"
            | "json"
            | "jsx"
            | "kt"
            | "md"
            | "py"
            | "rs"
            | "sh"
            | "toml"
            | "ts"
            | "tsx"
            | "yaml"
            | "yml"
    )
}

fn runtime_mem_prompt_term_is_symbol(term: &str) -> bool {
    if term.len() < 3 || runtime_mem_prompt_term_is_path(term) {
        return false;
    }
    let symbol = term.trim_end_matches("()");
    symbol
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.'))
        && (symbol.contains("::")
            || symbol
                .chars()
                .any(|ch| ch == '_' || ch.is_ascii_uppercase()))
}

fn runtime_mem_push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    let path = runtime_mem_normalize_recall_intent_path(&path);
    if path.as_os_str().is_empty() {
        return;
    }
    let normalized_path = runtime_mem_normalized_path(&path);
    if paths
        .iter()
        .any(|existing| runtime_mem_normalized_path(existing) == normalized_path)
    {
        return;
    }
    paths.push(path);
}

fn runtime_mem_push_unique_symbol(symbols: &mut Vec<String>, symbol: &str) {
    let symbol = runtime_mem_display_symbol(symbol);
    let normalized_symbol = runtime_mem_normalized_symbol(&symbol);
    if normalized_symbol.is_empty()
        || symbols
            .iter()
            .any(|existing| runtime_mem_normalized_symbol(existing) == normalized_symbol)
    {
        return;
    }
    symbols.push(symbol);
}

fn runtime_mem_normalize_recall_intent_path(path: &Path) -> PathBuf {
    path.to_str()
        .map(runtime_mem_strip_path_locator_suffix)
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

fn runtime_mem_display_symbol(symbol: &str) -> String {
    symbol
        .trim()
        .trim_matches('`')
        .trim_end_matches("()")
        .to_string()
}

fn runtime_mem_normalized_symbol(symbol: &str) -> String {
    runtime_mem_display_symbol(symbol).to_ascii_lowercase()
}

pub(super) fn runtime_mem_normalized_path(path: &Path) -> PathBuf {
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
