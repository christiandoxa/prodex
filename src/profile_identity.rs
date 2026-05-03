pub(crate) use super::shared_codex_fs::{persist_login_home, remove_dir_if_exists};
use super::*;

pub(crate) use prodex_profile_identity::{ProfileIdentity, parse_identity_from_auth_json};
#[cfg(test)]
pub(crate) use prodex_profile_identity::{parse_email_from_id_token, profile_name_from_email};

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
    let discovered = collect_profile_identities(state);
    Ok(prodex_profile_identity::find_matching_profile_identity(
        &discovered,
        target,
    ))
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
    prodex_profile_identity::unique_profile_name_for_email(email, |candidate| {
        reclaim_stale_managed_profile_path(paths, state, candidate);
        is_available_profile_name(paths, state, candidate)
    })
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
    fn find_profile_by_identity_matches_same_workspace_account_id_and_email() {
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
                email: Some("user@example.com".to_string()),
                account_id: Some("acct-one".to_string()),
            },
        )
        .unwrap();

        assert_eq!(matched.as_deref(), Some("first"));
    }

    #[test]
    fn find_profile_by_identity_does_not_match_same_workspace_with_different_email() {
        let root = temp_dir("identity-same-workspace-different-email");
        let first_home = root.join("first");
        fs::create_dir_all(&first_home).unwrap();
        fs::write(
            secret_store::auth_json_path(&first_home),
            format!(
                r#"{{"tokens":{{"id_token":"{}","access_token":"test-token","account_id":"acct-one"}}}}"#,
                chatgpt_id_token("customeradroit@gmail.com", Some("acct-one"))
            ),
        )
        .unwrap();
        let mut state = AppState {
            active_profile: None,
            profiles: BTreeMap::from([(
                "customeradroit_gmail.com".to_string(),
                ProfileEntry {
                    codex_home: first_home,
                    managed: true,
                    email: Some("customeradroit@gmail.com".to_string()),
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
                email: Some("usahaqteam@gmail.com".to_string()),
                account_id: Some("acct-one".to_string()),
            },
        )
        .unwrap();

        assert_eq!(matched, None);
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
