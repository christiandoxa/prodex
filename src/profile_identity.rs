pub(crate) use super::shared_codex_fs::{persist_login_home, remove_dir_if_exists};
use super::*;

#[allow(unused_imports)]
pub(crate) use prodex_profile_identity::{
    ProfileIdentity, normalize_account_id, normalize_email, parse_account_id_from_access_token,
    parse_email_from_auth_json, parse_email_from_id_token, parse_identity_from_auth_json,
    parse_identity_from_id_token, parse_jwt_payload, profile_name_from_email,
};

#[derive(Debug)]
struct ProfileIdentityLookupJob {
    name: String,
    codex_home: PathBuf,
    cached_email: Option<String>,
}

pub(crate) fn fetch_profile_email(codex_home: &Path) -> Result<String> {
    let auth_email_error = match read_profile_identity_from_auth(codex_home) {
        Ok(identity) if identity.email.is_some() => {
            return Ok(identity.email.unwrap_or_default());
        }
        Ok(_) => None,
        Err(err) => Some(err),
    };

    if let Some(model_provider) = codex_non_openai_model_provider(codex_home, None) {
        if let Some(auth_error) = auth_email_error {
            bail!(
                "failed to read account email from stored auth secret ({auth_error:#}); quota email fallback is unavailable for model_provider '{}'",
                model_provider.provider_id,
            );
        }
        bail!(
            "stored auth secret did not contain an account email and quota email fallback is unavailable for model_provider '{}'",
            model_provider.provider_id,
        );
    }

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

pub(crate) fn fetch_profile_identity(codex_home: &Path) -> Result<ProfileIdentity> {
    let (mut identity, auth_identity_error) = match read_profile_identity_from_auth(codex_home) {
        Ok(identity) => (identity, None),
        Err(err) => (ProfileIdentity::default(), Some(err)),
    };

    if identity.has_email() && identity.has_account_id() {
        return Ok(identity);
    }

    if let Some(model_provider) = codex_non_openai_model_provider(codex_home, None) {
        if let Some(auth_error) = auth_identity_error {
            bail!(
                "failed to read account identity from stored auth secret ({auth_error:#}); quota identity fallback is unavailable for model_provider '{}'",
                model_provider.provider_id,
            );
        }
        return Ok(identity);
    }

    if !identity.has_email() {
        match fetch_profile_email_from_usage(codex_home) {
            Ok(email) => identity.email = Some(email),
            Err(usage_error) => {
                if let Some(auth_error) = auth_identity_error {
                    bail!(
                        "failed to read account identity from stored auth secret ({auth_error:#}) and quota endpoint ({usage_error:#})"
                    );
                }
            }
        }
    }

    Ok(identity)
}

pub(crate) fn read_profile_account_id_from_auth(codex_home: &Path) -> Result<Option<String>> {
    Ok(read_profile_identity_from_auth(codex_home)?.account_id)
}

fn read_profile_identity_from_auth(codex_home: &Path) -> Result<ProfileIdentity> {
    let auth_location = secret_store::auth_json_path(codex_home);

    let Some(content) = read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_location.display()))?
    else {
        return Ok(ProfileIdentity::default());
    };
    parse_identity_from_auth_json(&content).with_context(|| {
        format!(
            "failed to parse account identity from {}",
            auth_location.display()
        )
    })
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

pub(crate) fn find_profile_by_identity(
    state: &mut AppState,
    target: &ProfileIdentity,
) -> Result<Option<String>> {
    let target_account_id = target.account_id.as_deref().map(normalize_account_id);
    let target_email = target.email.as_deref().map(normalize_email);
    let discovered = collect_profile_identities(state);

    if let Some(target_account_id) = target_account_id.as_deref() {
        for (name, identity) in &discovered {
            if identity
                .account_id
                .as_deref()
                .is_some_and(|account_id| normalize_account_id(account_id) == target_account_id)
            {
                return Ok(Some(name.clone()));
            }
        }
    }

    let Some(target_email) = target_email.as_deref() else {
        return Ok(None);
    };
    let legacy_email_matches = discovered
        .iter()
        .filter(|(_, identity)| {
            identity.account_id.is_none()
                && identity
                    .email
                    .as_deref()
                    .is_some_and(|email| normalize_email(email) == target_email)
        })
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    if legacy_email_matches.len() == 1 {
        Ok(legacy_email_matches.into_iter().next())
    } else {
        Ok(None)
    }
}

fn collect_profile_identities(state: &mut AppState) -> Vec<(String, ProfileIdentity)> {
    let jobs = state
        .profiles
        .iter()
        .filter_map(|(name, profile)| {
            if !profile.provider.supports_codex_runtime() {
                return None;
            }
            Some(ProfileIdentityLookupJob {
                name: name.clone(),
                codex_home: profile.codex_home.clone(),
                cached_email: profile.email.clone(),
            })
        })
        .collect();

    let discovered = map_parallel(jobs, |job| {
        let mut identity = fetch_profile_identity(&job.codex_home).unwrap_or_default();
        if identity.email.is_none() {
            identity.email = job.cached_email;
        }
        (job.name, identity)
    });

    for (name, identity) in &discovered {
        if let Some(email) = identity.email.as_ref()
            && let Some(profile) = state.profiles.get_mut(name)
        {
            profile.email = Some(email.clone());
        }
    }

    discovered
}

#[allow(dead_code)]
pub(crate) fn find_profile_by_email(state: &mut AppState, email: &str) -> Result<Option<String>> {
    find_profile_by_identity(
        state,
        &ProfileIdentity {
            email: Some(email.to_string()),
            account_id: None,
        },
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_profile_email_uses_auth_email_for_non_openai_model_provider() {
        let root = temp_dir("non-openai-auth-email");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();
        fs::write(
            secret_store::auth_json_path(&root),
            format!(
                r#"{{"tokens":{{"id_token":"{}","access_token":"test-token"}}}}"#,
                chatgpt_id_token("user@example.com", None)
            ),
        )
        .unwrap();

        let email = fetch_profile_email(&root).unwrap();

        assert_eq!(email, "user@example.com");
    }

    #[test]
    fn fetch_profile_email_skips_usage_fallback_for_non_openai_model_provider() {
        let root = temp_dir("non-openai-no-auth-email");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();
        fs::write(
            secret_store::auth_json_path(&root),
            r#"{"tokens":{"access_token":"test-token"}}"#,
        )
        .unwrap();

        let err = fetch_profile_email(&root).unwrap_err();
        let message = format!("{err:#}");

        assert!(message.contains("amazon-bedrock"));
        assert!(message.contains("quota email fallback is unavailable"));
    }

    #[test]
    fn parse_identity_from_auth_json_reads_email_and_workspace_account_id() {
        let identity = parse_identity_from_auth_json(&format!(
            r#"{{
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "{}",
                    "account_id": "stored-account"
                }}
            }}"#,
            chatgpt_id_token("user@example.com", Some("id-token-account")),
            chatgpt_access_token("access-token-account"),
        ))
        .unwrap();

        assert_eq!(identity.email.as_deref(), Some("user@example.com"));
        assert_eq!(identity.account_id.as_deref(), Some("id-token-account"));
    }

    #[test]
    fn parse_identity_from_auth_json_uses_access_token_before_stored_account_id() {
        let identity = parse_identity_from_auth_json(&format!(
            r#"{{
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "{}",
                    "account_id": "stored-account"
                }}
            }}"#,
            chatgpt_id_token("user@example.com", None),
            chatgpt_access_token("access-token-account"),
        ))
        .unwrap();

        assert_eq!(identity.account_id.as_deref(), Some("access-token-account"));
    }

    #[test]
    fn find_profile_by_identity_does_not_match_different_workspace_with_same_email() {
        let root = temp_dir("identity-different-workspace");
        let first_home = root.join("first");
        fs::create_dir_all(&first_home).unwrap();
        fs::write(
            secret_store::auth_json_path(&first_home),
            format!(
                r#"{{"tokens":{{"id_token":"{}","access_token":"test-token","account_id":"acct-one"}}}}"#,
                chatgpt_id_token("user@example.com", Some("acct-one"))
            ),
        )
        .unwrap();
        let mut state = AppState {
            active_profile: None,
            profiles: BTreeMap::from([(
                "first".to_string(),
                ProfileEntry {
                    codex_home: first_home,
                    managed: true,
                    email: Some("user@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        };

        let matched = find_profile_by_identity(
            &mut state,
            &ProfileIdentity {
                email: Some("user@example.com".to_string()),
                account_id: Some("acct-two".to_string()),
            },
        )
        .unwrap();

        assert_eq!(matched, None);
    }

    #[test]
    fn find_profile_by_identity_matches_same_workspace_account_id() {
        let root = temp_dir("identity-same-workspace");
        let first_home = root.join("first");
        fs::create_dir_all(&first_home).unwrap();
        fs::write(
            secret_store::auth_json_path(&first_home),
            format!(
                r#"{{"tokens":{{"id_token":"{}","access_token":"test-token","account_id":"acct-one"}}}}"#,
                chatgpt_id_token("user@example.com", Some("acct-one"))
            ),
        )
        .unwrap();
        let mut state = AppState {
            active_profile: None,
            profiles: BTreeMap::from([(
                "first".to_string(),
                ProfileEntry {
                    codex_home: first_home,
                    managed: true,
                    email: Some("user@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        };

        let matched = find_profile_by_identity(
            &mut state,
            &ProfileIdentity {
                email: Some("other@example.com".to_string()),
                account_id: Some("acct-one".to_string()),
            },
        )
        .unwrap();

        assert_eq!(matched.as_deref(), Some("first"));
    }

    fn chatgpt_id_token(email: &str, account_id: Option<&str>) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"alg":"none","typ":"JWT"}"#);
        let mut auth = serde_json::Map::new();
        if let Some(account_id) = account_id {
            auth.insert(
                "chatgpt_account_id".to_string(),
                serde_json::Value::String(account_id.to_string()),
            );
        }
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "https://api.openai.com/profile": {
                    "email": email
                },
                "https://api.openai.com/auth": auth,
            })
            .to_string(),
        );
        format!("{header}.{payload}.sig")
    }

    fn chatgpt_access_token(account_id: &str) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": account_id,
                },
            })
            .to_string(),
        );
        format!("{header}.{payload}.sig")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "prodex-profile-identity-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        dir
    }
}
