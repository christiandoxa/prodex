use super::*;

pub(super) fn runtime_mem_super_slim_v2_inline_dictionary_terms(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for token in runtime_mem_super_slim_v2_dictionary_tokens(value) {
        for term in runtime_mem_super_slim_v2_token_dictionary_terms(&token) {
            runtime_mem_push_dictionary_term(&mut terms, term);
        }
    }
    for term in runtime_mem_super_slim_v2_package_terms(value) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    terms
}

fn runtime_mem_super_slim_v2_dictionary_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
                )
        })
        .filter_map(|token| {
            let token = runtime_mem_super_slim_v2_trim_dictionary_token(token);
            (!token.is_empty()).then(|| token.to_string())
        })
        .collect()
}

fn runtime_mem_super_slim_v2_trim_dictionary_token(token: &str) -> &str {
    token
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        })
        .trim_end_matches(['.', ',', ';', '!', '?'])
}

fn runtime_mem_super_slim_v2_token_dictionary_terms(token: &str) -> Vec<String> {
    let mut terms = Vec::new();
    if runtime_mem_super_slim_v2_contains_dictionary_ref_marker(token) {
        return terms;
    }
    if let Some(term) = runtime_mem_super_slim_v2_temp_path_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    for term in runtime_mem_super_slim_v2_url_terms(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    if let Some(term) = runtime_mem_super_slim_v2_identity_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    if let Some(term) = runtime_mem_super_slim_v2_stack_prefix_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    terms
}

fn runtime_mem_super_slim_v2_temp_path_term(token: &str) -> Option<String> {
    let token = token.trim_end_matches(':');
    if ![
        "/tmp/",
        "/private/tmp/",
        "/var/tmp/",
        "/var/folders/",
        "/run/user/",
        "/dev/shm/",
    ]
    .iter()
    .any(|prefix| token.starts_with(prefix))
    {
        return None;
    }
    runtime_mem_super_slim_v2_path_prefix_term(token).or_else(|| {
        (token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| token.to_string())
    })
}

fn runtime_mem_super_slim_v2_path_prefix_term(token: &str) -> Option<String> {
    let slash_index = token.rfind('/')?;
    if slash_index == 0 {
        return None;
    }
    let prefix = &token[..=slash_index];
    (prefix.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| prefix.to_string())
}

fn runtime_mem_super_slim_v2_url_terms(token: &str) -> Vec<String> {
    if !token.starts_with("https://") && !token.starts_with("http://") {
        return Vec::new();
    }
    let mut terms = Vec::new();
    let token = token.trim_end_matches('/');
    if token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        runtime_mem_push_dictionary_term(&mut terms, token.to_string());
    }
    let scheme_end = token.find("://").map(|index| index + 3).unwrap_or_default();
    if let Some(path_start) = token[scheme_end..].find('/') {
        let origin_end = scheme_end + path_start;
        let origin = &token[..origin_end];
        if origin.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
            runtime_mem_push_dictionary_term(&mut terms, origin.to_string());
        }
        if let Some(prefix) = runtime_mem_super_slim_v2_path_prefix_term(token) {
            runtime_mem_push_dictionary_term(&mut terms, prefix);
        }
    }
    terms
}

fn runtime_mem_super_slim_v2_identity_term(token: &str) -> Option<String> {
    let token = token
        .trim_end_matches(':')
        .trim_start_matches("profile=")
        .trim_start_matches("account=")
        .trim_start_matches("ref=")
        .trim_start_matches("branch=");
    if token.len() < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        return None;
    }
    let lower = token.to_ascii_lowercase();
    let profile_like = [
        "profile-",
        "profile_",
        "prodex-profile-",
        "codex-profile-",
        "account-",
        "account_",
        "acct-",
        "acct_",
        "org-",
        "org_",
        "proj-",
        "proj_",
        "refs/heads/",
        "refs/remotes/",
        "refs/tags/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let branch_like = token.contains('/')
        && [
            "origin/",
            "upstream/",
            "feature/",
            "bugfix/",
            "hotfix/",
            "release/",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    let git_hash = token.len() >= 12 && token.chars().all(|ch| ch.is_ascii_hexdigit());
    (profile_like || branch_like || git_hash).then(|| token.to_string())
}

fn runtime_mem_super_slim_v2_stack_prefix_term(token: &str) -> Option<String> {
    if token.matches("::").count() < 2 {
        return None;
    }
    let prefix_end = token.rfind("::")? + 2;
    let prefix = &token[..prefix_end];
    (prefix.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| prefix.to_string())
}

fn runtime_mem_super_slim_v2_package_terms(value: &str) -> Vec<String> {
    let tokens = runtime_mem_super_slim_v2_dictionary_tokens(value);
    let mut terms = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let token = token
            .trim_start_matches("crate=")
            .trim_start_matches("package=")
            .trim_start_matches("pkg=");
        let previous = index.checked_sub(1).and_then(|index| tokens.get(index));
        let package_context = previous.is_some_and(|previous| {
            matches!(
                previous.as_str(),
                "-p" | "--package" | "--pkg" | "--crate" | "crate" | "package" | "pkg"
            )
        }) || value.contains(&format!("crate {token}"))
            || value.contains(&format!("package {token}"));
        let scoped_package = token.starts_with('@') && token.contains('/');
        let crate_like = package_context
            && token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS
            && token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '/' | '@'));
        if scoped_package || crate_like {
            runtime_mem_push_dictionary_term(&mut terms, token.to_string());
        }
    }
    terms
}
