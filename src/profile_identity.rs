use super::shared_codex_fs::copy_codex_home;
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProfileIdentity {
    pub(crate) email: Option<String>,
    pub(crate) account_id: Option<String>,
}

impl ProfileIdentity {
    fn has_email(&self) -> bool {
        self.email
            .as_deref()
            .is_some_and(|email| !email.trim().is_empty())
    }

    fn has_account_id(&self) -> bool {
        self.account_id
            .as_deref()
            .is_some_and(|account_id| !account_id.trim().is_empty())
    }
}

#[derive(Debug)]
struct ProfileIdentityLookupJob {
    name: String,
    codex_home: PathBuf,
    cached_email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenAccountClaims {
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<TokenAccountAuthClaims>,
    #[serde(rename = "https://api.openai.com/auth.chatgpt_account_id", default)]
    auth_chatgpt_account_id: Option<String>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

impl TokenAccountClaims {
    fn into_account_id(self) -> Option<String> {
        self.auth
            .and_then(|auth| auth.chatgpt_account_id)
            .or(self.auth_chatgpt_account_id)
            .or(self.chatgpt_account_id)
            .and_then(normalize_optional_account_id)
    }
}

#[derive(Debug, Deserialize)]
struct TokenAccountAuthClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
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

pub(crate) fn parse_identity_from_auth_json(raw_auth_json: &str) -> Result<ProfileIdentity> {
    let stored_auth: StoredAuth =
        serde_json::from_str(raw_auth_json).context("failed to parse auth JSON")?;
    parse_identity_from_stored_auth(&stored_auth)
}

#[allow(dead_code)]
pub(crate) fn parse_email_from_auth_json(raw_auth_json: &str) -> Result<Option<String>> {
    Ok(parse_identity_from_auth_json(raw_auth_json)?.email)
}

fn parse_identity_from_stored_auth(stored_auth: &StoredAuth) -> Result<ProfileIdentity> {
    let Some(tokens) = stored_auth.tokens.as_ref() else {
        return Ok(ProfileIdentity::default());
    };

    let id_token_identity = tokens
        .id_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(parse_identity_from_id_token)
        .transpose()?
        .unwrap_or_default();
    let stored_account_id = tokens
        .account_id
        .as_deref()
        .and_then(normalize_optional_account_id);
    let access_token_account_id = tokens
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .and_then(|token| parse_account_id_from_access_token(token).ok().flatten());

    Ok(ProfileIdentity {
        email: id_token_identity.email,
        account_id: id_token_identity
            .account_id
            .or(access_token_account_id)
            .or(stored_account_id),
    })
}

fn parse_identity_from_id_token(raw_jwt: &str) -> Result<ProfileIdentity> {
    let claims: IdTokenClaims = parse_jwt_payload(raw_jwt)?;
    Ok(ProfileIdentity {
        email: claims
            .email
            .or_else(|| claims.profile.and_then(|profile| profile.email))
            .and_then(normalize_optional_email),
        account_id: claims
            .auth
            .and_then(|auth| auth.chatgpt_account_id)
            .and_then(normalize_optional_account_id),
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_email_from_id_token(raw_jwt: &str) -> Result<Option<String>> {
    Ok(parse_identity_from_id_token(raw_jwt)?.email)
}

fn parse_account_id_from_access_token(raw_jwt: &str) -> Result<Option<String>> {
    let claims: TokenAccountClaims = parse_jwt_payload(raw_jwt)?;
    Ok(claims.into_account_id())
}

fn parse_jwt_payload<T>(raw_jwt: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
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
    serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")
}

fn normalize_optional_email(email: String) -> Option<String> {
    let email = email.trim().to_string();
    (!email.is_empty()).then_some(email)
}

fn normalize_optional_account_id(account_id: impl AsRef<str>) -> Option<String> {
    let account_id = account_id.as_ref().trim().to_string();
    (!account_id.is_empty()).then_some(account_id)
}

fn normalize_account_id(account_id: &str) -> String {
    account_id.trim().to_string()
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

pub(crate) fn normalize_email(email: &str) -> String {
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
