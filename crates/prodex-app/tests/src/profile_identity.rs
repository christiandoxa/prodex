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

#[test]
fn find_profile_by_identity_rejects_email_derived_name_for_other_account_email() {
    let root = temp_dir("identity-email-derived-name-mismatch");
    let first_home = root.join("first");
    fs::create_dir_all(&first_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&first_home),
        format!(
            r#"{{"tokens":{{"id_token":"{}","access_token":"test-token","account_id":"acct-one"}}}}"#,
            chatgpt_id_token("usahaqteam@gmail.com", Some("acct-one"))
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
                email: Some("usahaqteam@gmail.com".to_string()),
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
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
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
