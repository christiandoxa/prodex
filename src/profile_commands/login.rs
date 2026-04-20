use super::*;

pub(crate) fn handle_codex_login(args: CodexPassthroughArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let status = if let Some(profile_name) = args.profile.as_deref() {
        login_into_profile(&paths, &mut state, profile_name, &args.codex_args)?
    } else {
        login_with_auto_profile(&paths, &mut state, &args.codex_args)?
    };
    exit_with_status(status)
}

fn login_into_profile(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    codex_args: &[OsString],
) -> Result<ExitStatus> {
    let profile_name = resolve_profile_name(state, Some(profile_name))?;
    let codex_home = prepare_profile_login_home(paths, state, &profile_name)?;
    let status = run_codex_login(&codex_home, codex_args)?;
    if !status.success() {
        return Ok(status);
    }

    finish_named_profile_login(paths, state, &profile_name, &codex_home)?;
    Ok(status)
}

fn prepare_profile_login_home(
    paths: &AppPaths,
    state: &AppState,
    profile_name: &str,
) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !profile.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex login --profile` currently supports OpenAI/Codex profiles only.",
            profile_name,
            profile.provider.display_name()
        );
    }
    let codex_home = profile.codex_home.clone();
    if profile.managed {
        prepare_managed_codex_home(paths, &codex_home)?;
    } else {
        create_codex_home_if_missing(&codex_home)?;
    }
    Ok(codex_home)
}

fn finish_named_profile_login(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    codex_home: &Path,
) -> Result<()> {
    refresh_profile_email_from_home(state, profile_name, codex_home);
    let account_email = profile_email_label(state, profile_name);
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in successfully for profile '{profile_name}'."),
        ),
        ("Account".to_string(), account_email),
        ("Profile".to_string(), profile_name.to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn refresh_profile_email_from_home(state: &mut AppState, profile_name: &str, codex_home: &Path) {
    if let Ok(email) = fetch_profile_email(codex_home)
        && let Some(profile) = state.profiles.get_mut(profile_name)
    {
        profile.email = Some(email);
    }
}

fn profile_email_label(state: &AppState, profile_name: &str) -> String {
    state
        .profiles
        .get(profile_name)
        .and_then(|profile| profile.email.clone())
        .unwrap_or_else(|| "-".to_string())
}

fn login_with_auto_profile(
    paths: &AppPaths,
    state: &mut AppState,
    codex_args: &[OsString],
) -> Result<ExitStatus> {
    let login_home = create_temporary_login_home(paths)?;
    let status = run_codex_login(&login_home, codex_args)?;
    if !status.success() {
        remove_dir_if_exists(&login_home)?;
        return Ok(status);
    }

    let email = fetch_profile_email(&login_home).with_context(|| {
        format!(
            "failed to resolve the logged-in account email from {}",
            login_home.display()
        )
    })?;
    let auth_json = required_auth_json_text(&login_home)?;

    if let Some(profile_name) = find_profile_by_email(state, &email)? {
        finish_auto_login_for_existing_profile(
            paths,
            state,
            &login_home,
            &profile_name,
            &email,
            &auth_json,
        )?;
        return Ok(status);
    }

    finish_auto_login_for_new_profile(paths, state, &login_home, &email)?;
    Ok(status)
}

fn finish_auto_login_for_existing_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    profile_name: &str,
    email: &str,
    auth_json: &str,
) -> Result<()> {
    let updated =
        update_existing_profile_auth(paths, state, profile_name, Some(email), auth_json, true)?;
    remove_dir_if_exists(login_home)?;
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!(
                "Logged in as {email}. Updated auth token for existing profile '{}'.",
                updated.profile_name
            ),
        ),
        ("Account".to_string(), email.to_string()),
        ("Profile".to_string(), updated.profile_name),
        (
            "CODEX_HOME".to_string(),
            updated.codex_home.display().to_string(),
        ),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn finish_auto_login_for_new_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    email: &str,
) -> Result<()> {
    let profile_name = unique_profile_name_for_email(paths, state, email);
    let codex_home = managed_profile_home_path(paths, &profile_name)?;
    persist_login_home(login_home, &codex_home)?;
    prepare_managed_codex_home(paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(email.to_string()),
            provider: ProfileProvider::Openai,
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in as {email}. Created profile '{profile_name}'."),
        ),
        ("Account".to_string(), email.to_string()),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn run_codex_login(codex_home: &Path, codex_args: &[OsString]) -> Result<ExitStatus> {
    let mut command_args = vec![OsString::from("login")];
    command_args.extend(codex_args.iter().cloned());
    run_child_plan(
        &ChildProcessPlan::new(codex_bin(), codex_home.to_path_buf()).with_args(command_args),
        None,
    )
}

fn create_temporary_login_home(paths: &AppPaths) -> Result<PathBuf> {
    ensure_managed_profiles_root(paths)?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = paths
            .managed_profiles_root
            .join(format!(".login-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for login")
}

pub(crate) fn handle_codex_logout(args: LogoutArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.selected_profile())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !codex_home.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex logout` currently supports OpenAI/Codex profiles only.",
            profile_name,
            codex_home.provider.display_name()
        );
    }
    let codex_home = codex_home.codex_home.clone();

    let status = run_child_plan(
        &ChildProcessPlan::new(codex_bin(), codex_home.clone())
            .with_args(vec![OsString::from("logout")]),
        None,
    )?;
    exit_with_status(status)
}
