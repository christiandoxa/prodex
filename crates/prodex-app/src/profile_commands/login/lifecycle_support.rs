use super::super::import_export::{
    ProfileAuthUpdate, acquire_profile_lifecycle_lock, cleanup_profile_lifecycle_and_auth_journal,
    load_profile_state_with_profile_recovery_locked, prepare_existing_profile_lifecycle,
    read_optional_secret_text_file,
};
use super::{
    LoginMethod, LoginRequest, create_temporary_login_home, fetch_profile_email,
    finish_named_anthropic_profile_login, finish_named_profile_login,
    prepare_anthropic_profile_login_home, prepare_profile_login_home, read_auth_summary,
    required_auth_json_text, run_codex_login, write_secret_text_file,
};
use crate::{
    AppPaths, AppState, ProfileEntry, ProfileProvider, claude_external_oauth_profile_identity,
    read_external_claude_credentials_text, remove_dir_if_exists, resolve_profile_name,
};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::ExitStatus;

#[derive(Debug, Clone)]
struct ProfileLoginTarget {
    name: String,
    profile: ProfileEntry,
}

fn validate_profile_login_target(state: &AppState, target: &ProfileLoginTarget) -> Result<()> {
    let Some(current) = state.profiles.get(&target.name) else {
        bail!("profile '{}' changed while login was running", target.name);
    };
    if current != &target.profile {
        bail!("profile '{}' changed while login was running", target.name);
    }
    Ok(())
}

fn validate_profile_login_request(
    state: &AppState,
    profile_name: &str,
    method: LoginMethod,
) -> Result<()> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if method == LoginMethod::Claude {
        if matches!(
            profile.provider,
            ProfileProvider::Gemini { .. }
                | ProfileProvider::Copilot { .. }
                | ProfileProvider::Agy { .. }
        ) {
            bail!(
                "profile '{}' uses {}. Claude sign-in supports OpenAI/Codex placeholders or Anthropic Claude profiles.",
                profile_name,
                profile.provider.display_name()
            );
        }
    } else if !profile.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex login --profile` currently supports OpenAI/Codex profiles only.",
            profile_name,
            profile.provider.display_name()
        );
    }
    Ok(())
}

pub(super) fn login_into_profile(
    paths: &AppPaths,
    requested_profile_name: &str,
    login_request: &LoginRequest,
) -> Result<ExitStatus> {
    // ponytail: one global lock closes remove/recreate races; use per-profile locks if login concurrency matters.
    let _lock = acquire_profile_lifecycle_lock(paths)?;
    let target = {
        let (state, _) = load_profile_state_with_profile_recovery_locked(paths, true)?;
        let profile_name = resolve_profile_name(&state, Some(requested_profile_name))?;
        validate_profile_login_request(&state, &profile_name, login_request.method)?;
        let profile = state
            .profiles
            .get(&profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?
            .clone();
        ProfileLoginTarget {
            name: profile_name,
            profile,
        }
    };

    if login_request.method == LoginMethod::Status {
        return run_codex_login(&target.profile.codex_home, login_request);
    }

    let login_home = create_temporary_login_home(paths)?;
    let status = run_codex_login(&login_home, login_request)?;
    if !status.success() {
        remove_dir_if_exists(&login_home)?;
        return Ok(status);
    }

    let (mut state, _) = load_profile_state_with_profile_recovery_locked(paths, true)?;
    if let Err(err) = validate_profile_login_target(&state, &target) {
        remove_dir_if_exists(&login_home)?;
        return Err(err);
    }
    validate_profile_login_request(&state, &target.name, login_request.method)?;
    finish_login_into_profile_locked(
        paths,
        &mut state,
        &target.name,
        login_request,
        &login_home,
        status,
    )
}

fn finish_login_into_profile_locked(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    login_request: &LoginRequest,
    login_home: &Path,
    status: ExitStatus,
) -> Result<ExitStatus> {
    if login_request.method == LoginMethod::Claude {
        let codex_home = prepare_anthropic_profile_login_home(paths, state, profile_name)?;
        let (account, auth_method) = claude_external_oauth_profile_identity(login_home)?;
        let mut desired_profile = state
            .profiles
            .get(profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?
            .clone();
        desired_profile.email = account.clone();
        desired_profile.provider = ProfileProvider::Anthropic {
            account,
            auth_method,
        };
        let credentials = read_external_claude_credentials_text(login_home)
            .context("Claude login did not produce credentials")?;
        let (lifecycle_path, auth_journal_path) = prepare_existing_profile_lifecycle(
            paths,
            "login",
            state,
            profile_name,
            &desired_profile,
            Some(profile_name.to_string()),
            ProfileAuthUpdate {
                next_auth_json: None,
                next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
                next_secret_files: vec![prodex_profile_export::ImportedExistingProfileFileUpdate {
                    path: crate::CLAUDE_CREDENTIALS_FILE.to_string(),
                    text: Some(credentials),
                }],
                previous_secret_file_paths: &[crate::CLAUDE_CREDENTIALS_FILE],
                temporary_home: Some(login_home),
            },
        )?;
        crate::copy_claude_oauth_credentials(login_home, &codex_home)?;
        finish_named_anthropic_profile_login(paths, state, profile_name, &codex_home)?;
        remove_dir_if_exists(login_home)?;
        cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;
        return Ok(status);
    }

    let codex_home = prepare_profile_login_home(paths, state, profile_name)?;
    let auth_json = required_auth_json_text(login_home)?;
    let auth_label = read_auth_summary(login_home).label;
    let mut desired_profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .clone();
    desired_profile.email = if auth_label == "api-key" {
        None
    } else {
        fetch_profile_email(login_home)
            .ok()
            .or(desired_profile.email)
    };
    let (lifecycle_path, auth_journal_path) = prepare_existing_profile_lifecycle(
        paths,
        "login",
        state,
        profile_name,
        &desired_profile,
        Some(profile_name.to_string()),
        ProfileAuthUpdate {
            next_auth_json: Some(auth_json.clone()),
            next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
            next_secret_files: if login_request.openai_base_url_specified {
                vec![prodex_profile_export::ImportedExistingProfileFileUpdate {
                    path: ".prodex-profile.toml".to_string(),
                    text: read_optional_secret_text_file(&login_home.join(".prodex-profile.toml"))?,
                }]
            } else {
                Vec::new()
            },
            previous_secret_file_paths: if login_request.openai_base_url_specified {
                &[".prodex-profile.toml"][..]
            } else {
                &[][..]
            },
            temporary_home: Some(login_home),
        },
    )?;
    write_secret_text_file(&secret_store::auth_json_path(&codex_home), &auth_json)?;

    finish_named_profile_login(
        paths,
        state,
        profile_name,
        &codex_home,
        login_request.openai_base_url.as_deref(),
        login_request.openai_base_url_specified,
    )?;
    remove_dir_if_exists(login_home)?;
    cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;
    Ok(status)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{AppStateIoExt, TestEnvVarGuard, create_codex_home_if_missing};
    use std::fs;
    use std::sync::mpsc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn named_login_status_uses_selected_profile_home() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "prodex-login-status-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("test root should be private");
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy"),
        };
        fs::create_dir_all(&paths.managed_profiles_root)
            .expect("managed profiles root should be created");
        let profile_home = paths.managed_profiles_root.join("main");
        create_codex_home_if_missing(&profile_home).expect("profile home should be created");
        AppState {
            active_profile: Some("main".to_string()),
            profiles: std::collections::BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: profile_home.clone(),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        }
        .save(&paths)
        .expect("initial state should save");

        let script = root.join("fake-codex-login-status.sh");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s' \"$CODEX_HOME\" > \"$PRODEX_LOGIN_HOME_CAPTURE\"\n",
        )
        .expect("fake login command should be written");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700))
            .expect("fake login command should be executable");
        let capture = root.join("login-home");
        let _codex_guard = TestEnvVarGuard::set("PRODEX_CODEX_BIN", &script.display().to_string());
        let _capture_guard =
            TestEnvVarGuard::set("PRODEX_LOGIN_HOME_CAPTURE", &capture.display().to_string());

        let status = login_into_profile(
            &paths,
            "main",
            &LoginRequest {
                method: LoginMethod::Status,
                codex_args: Vec::new(),
                api_key: None,
                openai_base_url: None,
                openai_base_url_specified: false,
                api_key_profile_name: None,
            },
        )
        .expect("profile login status should run");

        assert!(status.success());
        assert_eq!(
            fs::read_to_string(&capture).expect("fake login should capture CODEX_HOME"),
            profile_home.display().to_string()
        );
        assert!(
            fs::read_dir(&paths.managed_profiles_root)
                .expect("managed profiles root should be readable")
                .filter_map(|entry| entry.ok())
                .all(|entry| !entry.file_name().to_string_lossy().starts_with(".login-")),
            "status should not allocate a temporary login home"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn named_login_rejects_profile_recreated_during_external_login() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "prodex-login-race-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("test root should be private");
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy"),
        };
        fs::create_dir_all(&paths.managed_profiles_root)
            .expect("managed profiles root should be created");
        let original_home = paths.managed_profiles_root.join("main");
        create_codex_home_if_missing(&original_home).expect("original home should be created");
        AppState {
            active_profile: Some("main".to_string()),
            profiles: std::collections::BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: original_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        }
        .save(&paths)
        .expect("initial state should save");

        let script = root.join("fake-codex-login.sh");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s' '{\"auth_mode\":\"chatgpt\",\"tokens\":{\"access_token\":\"temporary-token\",\"account_id\":\"temporary-account\"}}' > \"$CODEX_HOME/auth.json\"\ntouch \"$PRODEX_LOGIN_READY\"\nwhile [ ! -f \"$PRODEX_LOGIN_RELEASE\" ]; do sleep 0.01; done\n",
        )
        .expect("fake login command should be written");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700))
            .expect("fake login command should be executable");
        let ready = root.join("login-ready");
        let release = root.join("login-release");
        let _codex_guard = TestEnvVarGuard::set("PRODEX_CODEX_BIN", &script.display().to_string());
        let _ready_guard = TestEnvVarGuard::set("PRODEX_LOGIN_READY", &ready.display().to_string());
        let _release_guard =
            TestEnvVarGuard::set("PRODEX_LOGIN_RELEASE", &release.display().to_string());

        let login_paths = paths.clone();
        let login_thread = std::thread::spawn(move || {
            login_into_profile(
                &login_paths,
                "main",
                &LoginRequest {
                    method: LoginMethod::ChatGpt,
                    codex_args: Vec::new(),
                    api_key: None,
                    openai_base_url: None,
                    openai_base_url_specified: false,
                    api_key_profile_name: None,
                },
            )
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            if login_thread.is_finished() {
                panic!(
                    "login exited before fake command pause: {:#?}",
                    login_thread.join().expect("login thread should finish")
                );
            }
            assert!(
                Instant::now() < deadline,
                "fake login should reach its pause"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        let lock_paths = paths.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let lock_thread = std::thread::spawn(move || -> Result<()> {
            started_tx.send(()).expect("lock waiter should start");
            let _lock = acquire_profile_lifecycle_lock(&lock_paths)?;
            acquired_tx.send(()).expect("lock waiter should report");
            Ok(())
        });
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("lock waiter should start");
        assert!(
            matches!(
                acquired_rx.recv_timeout(Duration::from_millis(100)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "profile lifecycle mutation must wait for external login"
        );

        let replacement_home = paths.managed_profiles_root.join("replacement-home");
        create_codex_home_if_missing(&replacement_home)
            .expect("replacement home should be created");
        let mut state = AppState::load(&paths).expect("state should load during login");
        state.profiles.remove("main");
        state.active_profile = None;
        state
            .save_with_removed_profiles(&paths, &["main".to_string()])
            .expect("profile removal should save during login");
        state.profiles.insert(
            "main".to_string(),
            ProfileEntry {
                codex_home: replacement_home.clone(),
                managed: true,
                email: Some("replacement@example.com".to_string()),
                provider: ProfileProvider::Gemini {
                    email: "replacement@example.com".to_string(),
                    project_id: None,
                },
            },
        );
        state.active_profile = Some("main".to_string());
        state.save(&paths).expect("replacement profile should save");
        fs::write(&release, "release").expect("fake login should be released");

        let error = login_thread
            .join()
            .expect("login thread should finish")
            .expect_err("recreated profile must reject temporary credentials");
        assert!(
            error
                .to_string()
                .contains("changed while login was running"),
            "unexpected login race error: {error:#}"
        );
        acquired_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("lifecycle lock should be released after login");
        lock_thread
            .join()
            .expect("lock waiter should finish")
            .expect("lock waiter should acquire lifecycle lock");
        assert!(
            !replacement_home.join("auth.json").exists(),
            "temporary credentials must not be committed to the replacement profile"
        );
        assert!(
            fs::read_dir(&paths.managed_profiles_root)
                .expect("managed profiles root should be readable")
                .filter_map(|entry| entry.ok())
                .all(|entry| !entry.file_name().to_string_lossy().starts_with(".login-")),
            "temporary login home should be cleaned after identity mismatch"
        );
        let _ = fs::remove_dir_all(root);
    }
}
