use super::*;

pub(crate) fn normalize_intent_terms_with_prompt_expansion(terms: &[String]) -> Vec<String> {
    let mut expanded = Vec::<String>::new();
    for term in terms {
        if !term.trim().is_empty() && !term.chars().any(char::is_whitespace) {
            expanded.push(term.clone());
        }
        for extracted in extract_intent_terms_from_prompt(term) {
            expanded.push(extracted);
        }
        if expanded.len() >= MAX_EXTRACTED_INTENT_TERMS.saturating_mul(2) {
            break;
        }
    }
    normalize_intent_terms(&expanded)
        .into_iter()
        .take(MAX_EXTRACTED_INTENT_TERMS)
        .collect()
}

pub fn extract_intent_terms_from_prompt(prompt: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let mut token = String::new();
    let mut chars = prompt.chars().peekable();

    while let Some(ch) = chars.next() {
        if matches!(ch, '\'' | '"' | '`') {
            push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
            token.clear();

            let quote = ch;
            let mut quoted = String::new();
            for next in chars.by_ref() {
                if next == quote {
                    break;
                }
                quoted.push(next);
            }
            push_prompt_intent_candidate(&mut terms, &mut seen, &quoted, true);
        } else if is_prompt_intent_token_char(ch) {
            token.push(ch);
        } else {
            push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
            token.clear();
        }

        if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
            return terms;
        }
    }
    push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
    terms
}

fn normalize_intent_terms(terms: &[String]) -> Vec<String> {
    let mut seen = BTreeMap::<String, ()>::new();
    let mut normalized = Vec::new();
    for term in terms {
        let term = normalize_intent_match_text(term.trim());
        if term.is_empty() || seen.contains_key(&term) {
            continue;
        }
        seen.insert(term.clone(), ());
        normalized.push(term);
    }
    normalized
}

fn push_prompt_intent_candidate(
    terms: &mut Vec<String>,
    seen: &mut BTreeMap<String, ()>,
    candidate: &str,
    quoted: bool,
) {
    if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
        return;
    }
    if quoted && candidate.chars().any(char::is_whitespace) {
        for part in candidate.split_whitespace() {
            push_prompt_intent_candidate(terms, seen, part, false);
            if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
                break;
            }
        }
        return;
    }

    let Some(candidate) = normalize_prompt_intent_candidate(candidate) else {
        return;
    };

    let term = if let Some(code) = prompt_intent_error_code(&candidate) {
        Some(code)
    } else if let Some(path) = prompt_intent_path(&candidate) {
        Some(path)
    } else if looks_like_prompt_intent_file_name(&candidate)
        || looks_like_prompt_intent_symbol(&candidate)
        || quoted && looks_like_quoted_prompt_intent_identifier(&candidate)
    {
        Some(candidate)
    } else {
        None
    };

    let Some(term) = term else {
        return;
    };
    if term.chars().count() > 160 {
        return;
    }

    let key = normalize_intent_match_text(&term);
    if key.is_empty() || seen.contains_key(&key) {
        return;
    }
    seen.insert(key, ());
    terms.push(term);
}

fn normalize_prompt_intent_candidate(candidate: &str) -> Option<String> {
    let candidate = candidate.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ','
                | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '!'
                | '?'
                | '*'
                | '#'
                | '='
                | ':'
        )
    });
    let candidate = candidate.trim_end_matches(['.', ',']);
    if candidate.is_empty() || candidate.chars().any(char::is_whitespace) {
        return None;
    }

    Some(candidate.replace('\\', "/"))
}

fn is_prompt_intent_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '\\' | ':' | '$' | '[' | ']')
}

fn prompt_intent_error_code(candidate: &str) -> Option<String> {
    let chars = candidate.chars().collect::<Vec<_>>();
    for index in 0..chars.len() {
        if chars[index].eq_ignore_ascii_case(&'e')
            && index + 4 < chars.len()
            && chars[index + 1..=index + 4]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && prompt_intent_code_boundary(&chars, index.checked_sub(1))
            && prompt_intent_code_boundary(&chars, Some(index + 5))
        {
            return Some(
                chars[index..=index + 4]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            );
        }

        if chars[index].eq_ignore_ascii_case(&'t')
            && index + 5 < chars.len()
            && chars[index + 1].eq_ignore_ascii_case(&'s')
            && chars[index + 2..=index + 5]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && prompt_intent_code_boundary(&chars, index.checked_sub(1))
            && prompt_intent_code_boundary(&chars, Some(index + 6))
        {
            return Some(
                chars[index..=index + 5]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            );
        }
    }
    None
}

fn prompt_intent_code_boundary(chars: &[char], index: Option<usize>) -> bool {
    index.is_none_or(|index| {
        chars
            .get(index)
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && *ch != '_')
    })
}

fn prompt_intent_path(candidate: &str) -> Option<String> {
    if candidate.contains("://") || candidate.contains(' ') {
        return None;
    }

    let stripped = context_noise_strip_path_location_suffix_supplement(candidate)
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches('/');
    if stripped.len() < 3 || !stripped.contains('/') || !looks_like_location_path(stripped) {
        return None;
    }

    Some(stripped.to_string())
}

fn looks_like_prompt_intent_file_name(candidate: &str) -> bool {
    if candidate.contains('/') || candidate.contains("://") || candidate.starts_with('-') {
        return false;
    }
    let Some((stem, ext)) = candidate.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && stem.chars().any(|ch| ch.is_ascii_alphanumeric())
        && !ext.is_empty()
        && ext.len() <= 12
        && ext
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn looks_like_prompt_intent_symbol(candidate: &str) -> bool {
    if candidate.contains("://") || candidate.len() < 3 {
        return false;
    }

    if candidate.contains("::") {
        return candidate
            .split("::")
            .all(is_prompt_intent_identifier_segment);
    }

    if candidate.contains('.') && !looks_like_prompt_intent_file_name(candidate) {
        let segments = candidate.split('.').collect::<Vec<_>>();
        return segments.len() >= 2
            && segments
                .iter()
                .all(|segment| is_prompt_intent_identifier_segment(segment));
    }

    if !is_prompt_intent_identifier_segment(candidate) {
        return false;
    }
    let lower = candidate.to_ascii_lowercase();
    if is_noisy_prompt_intent_word(&lower) {
        return false;
    }
    candidate.starts_with("test_")
        || candidate.ends_with("_test")
        || candidate.contains('_')
        || candidate
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch == '_' || ch.is_ascii_digit())
        || has_prompt_intent_camel_shape(candidate)
}

fn looks_like_quoted_prompt_intent_identifier(candidate: &str) -> bool {
    if !is_prompt_intent_identifier_segment(candidate) {
        return false;
    }
    !is_noisy_prompt_intent_word(&candidate.to_ascii_lowercase())
}

fn is_prompt_intent_identifier_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn has_prompt_intent_camel_shape(candidate: &str) -> bool {
    let uppercase = candidate
        .chars()
        .filter(|ch| ch.is_ascii_uppercase())
        .count();
    let lowercase = candidate.chars().any(|ch| ch.is_ascii_lowercase());
    if uppercase == 0 || !lowercase {
        return false;
    }

    let mut previous_lowercase = false;
    for ch in candidate.chars() {
        if previous_lowercase && ch.is_ascii_uppercase() {
            return true;
        }
        previous_lowercase = ch.is_ascii_lowercase();
    }
    uppercase >= 2
}

fn is_noisy_prompt_intent_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "the"
            | "this"
            | "to"
            | "with"
            | "without"
            | "should"
            | "when"
            | "please"
            | "fix"
            | "add"
            | "update"
            | "change"
            | "make"
            | "use"
            | "using"
            | "run"
            | "check"
            | "test"
            | "tests"
            | "error"
            | "failed"
            | "failure"
            | "warning"
            | "output"
            | "command"
            | "context"
            | "smart"
            | "tool"
            | "compaction"
            | "prompt"
            | "request"
            | "ignore"
            | "common"
            | "word"
            | "words"
            | "file"
            | "files"
            | "repo"
            | "path"
            | "paths"
            | "rust"
            | "python"
            | "javascript"
            | "typescript"
    )
}
