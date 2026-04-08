use super::shared_codex_fs::copy_codex_home;
use super::*;

pub(crate) fn fetch_profile_email(codex_home: &Path) -> Result<String> {
    let auth_email_error = match read_profile_email_from_auth(codex_home) {
        Ok(Some(email)) => return Ok(email),
        Ok(None) => None,
        Err(err) => Some(err),
    };

    match fetch_profile_email_from_usage(codex_home) {
        Ok(email) => Ok(email),
        Err(usage_error) => {
            if let Some(auth_error) = auth_email_error {
                bail!(
                    "failed to read account email from stored auth secret ({auth_error:#}) and quota endpoint ({usage_error:#})"
                );
            }
            Err(usage_error)
        }
    }
}

fn read_profile_email_from_auth(codex_home: &Path) -> Result<Option<String>> {
    let auth_location = secret_store::auth_json_path(codex_home);

    let Some(content) = read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_location.display()))?
    else {
        return Ok(None);
    };
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_location.display()))?;
    let id_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.id_token.as_deref())
        .map(str::trim)
        .filter(|token| !token.is_empty());

    let Some(id_token) = id_token else {
        return Ok(None);
    };

    parse_email_from_id_token(id_token)
        .with_context(|| format!("failed to parse id_token in {}", auth_location.display()))
}

pub(crate) fn parse_email_from_id_token(raw_jwt: &str) -> Result<Option<String>> {
    let mut parts = raw_jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(payload), Some(signature))
            if !header.is_empty() && !payload.is_empty() && !signature.is_empty() =>
        {
            (header, payload, signature)
        }
        _ => bail!("invalid JWT format"),
    };

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload_b64))
        .context("failed to decode JWT payload")?;
    let claims: IdTokenClaims =
        serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")?;

    Ok(claims
        .email
        .or_else(|| claims.profile.and_then(|profile| profile.email))
        .map(|email| email.trim().to_string())
        .filter(|email| !email.is_empty()))
}

fn fetch_profile_email_from_usage(codex_home: &Path) -> Result<String> {
    let usage = fetch_usage(codex_home, None)?;
    let email = usage
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .context("quota endpoint did not return an email")?;
    Ok(email.to_string())
}

pub(crate) fn find_profile_by_email(state: &mut AppState, email: &str) -> Result<Option<String>> {
    let target_email = normalize_email(email);
    let summaries = collect_profile_summaries(state);

    for summary in &summaries {
        if summary
            .email
            .as_deref()
            .is_some_and(|cached| normalize_email(cached) == target_email)
        {
            return Ok(Some(summary.name.clone()));
        }
    }

    let discovered = map_parallel(
        summaries
            .into_iter()
            .filter_map(|summary| {
                if summary.email.is_some() || !summary.auth.quota_compatible {
                    return None;
                }

                Some(ProfileEmailLookupJob {
                    name: summary.name,
                    codex_home: summary.codex_home,
                })
            })
            .collect(),
        |job| (job.name, fetch_profile_email(&job.codex_home).ok()),
    );

    let mut matched_profile = None;
    for (name, fetched_email) in discovered {
        let Some(fetched_email) = fetched_email else {
            continue;
        };

        if matched_profile.is_none() && normalize_email(&fetched_email) == target_email {
            matched_profile = Some(name.clone());
        }
        if let Some(profile) = state.profiles.get_mut(&name) {
            profile.email = Some(fetched_email);
        }
    }

    Ok(matched_profile)
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

pub(crate) fn profile_name_from_email(email: &str) -> String {
    let normalized = normalize_email(email);
    let mut profile_name = String::new();

    for ch in normalized.chars() {
        match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => profile_name.push(ch),
            '@' => profile_name.push('_'),
            _ => profile_name.push('-'),
        }
    }

    let profile_name = profile_name
        .trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string();
    if profile_name.is_empty() || profile_name == "." || profile_name == ".." {
        "profile".to_string()
    } else {
        profile_name
    }
}

pub(crate) fn unique_profile_name_for_email(
    paths: &AppPaths,
    state: &AppState,
    email: &str,
) -> String {
    let base_name = profile_name_from_email(email);
    reclaim_stale_managed_profile_path(paths, state, &base_name);
    if is_available_profile_name(paths, state, &base_name) {
        return base_name;
    }

    for suffix in 2.. {
        let candidate = format!("{base_name}-{suffix}");
        reclaim_stale_managed_profile_path(paths, state, &candidate);
        if is_available_profile_name(paths, state, &candidate) {
            return candidate;
        }
    }

    unreachable!("integer suffix space should not be exhausted")
}

fn is_available_profile_name(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn reclaim_stale_managed_profile_path(paths: &AppPaths, state: &AppState, candidate: &str) {
    if state.profiles.contains_key(candidate) {
        return;
    }
    let candidate_path = paths.managed_profiles_root.join(candidate);
    if candidate_path.exists() {
        let _ = remove_dir_if_exists(&candidate_path);
    }
}

pub(crate) fn persist_login_home(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        bail!(
            "refusing to overwrite existing login destination {}",
            destination.display()
        );
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_codex_home(source, destination)?;
            remove_dir_if_exists(source)
        }
    }
}

pub(crate) fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).with_context(|| format!("failed to delete {}", path.display()))
}
