use super::profile_identity::{
    fetch_profile_email, find_profile_by_email, persist_login_home, remove_dir_if_exists,
    unique_profile_name_for_email,
};
use super::shared_codex_fs::{
    copy_codex_home, copy_directory_contents, create_codex_home_if_missing,
    prepare_managed_codex_home,
};
use super::*;

pub(crate) fn handle_add_profile(args: AddProfileArgs) -> Result<()> {
    validate_profile_name(&args.name)?;

    if args.codex_home.is_some() && (args.copy_from.is_some() || args.copy_current) {
        bail!("--codex-home cannot be combined with --copy-from or --copy-current");
    }

    if args.copy_from.is_some() && args.copy_current {
        bail!("use either --copy-from or --copy-current");
    }

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    if state.profiles.contains_key(&args.name) {
        bail!("profile '{}' already exists", args.name);
    }

    let managed = args.codex_home.is_none();
    let source_home = if args.copy_current {
        Some(default_codex_home(&paths)?)
    } else if let Some(path) = args.copy_from {
        Some(absolutize(path)?)
    } else {
        None
    };

    let codex_home = match args.codex_home {
        Some(path) => {
            let home = absolutize(path)?;
            create_codex_home_if_missing(&home)?;
            home
        }
        None => {
            fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
                format!(
                    "failed to create managed profile root {}",
                    paths.managed_profiles_root.display()
                )
            })?;
            let home = absolutize(paths.managed_profiles_root.join(&args.name))?;
            if let Some(source) = source_home.as_deref() {
                copy_codex_home(source, &home)?;
            } else {
                create_codex_home_if_missing(&home)?;
            }
            home
        }
    };

    if managed {
        prepare_managed_codex_home(&paths, &codex_home)?;
    }

    ensure_path_is_unique(&state, &codex_home)?;

    state.profiles.insert(
        args.name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed,
            email: None,
        },
    );

    if state.active_profile.is_none() || args.activate {
        state.active_profile = Some(args.name.clone());
    }

    state.save(&paths)?;

    let storage_message = if source_home.is_some() {
        "Source copied into managed profile home.".to_string()
    } else if managed {
        "Managed profile home created.".to_string()
    } else {
        "Existing CODEX_HOME registered.".to_string()
    };

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Added profile '{}'.", args.name),
        ),
        ("Profile".to_string(), args.name.clone()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
        ("Storage".to_string(), storage_message),
    ];
    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        fields.push(("Active".to_string(), args.name.clone()));
    }
    print_panel("Profile Added", &fields);

    Ok(())
}

pub(crate) fn handle_import_current_profile(args: ImportCurrentArgs) -> Result<()> {
    handle_add_profile(AddProfileArgs {
        name: args.name,
        codex_home: None,
        copy_from: None,
        copy_current: true,
        activate: true,
    })
}

pub(crate) fn handle_list_profiles() -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if state.profiles.is_empty() {
        let fields = vec![
            ("Status".to_string(), "No profiles configured.".to_string()),
            (
                "Create".to_string(),
                "prodex profile add <name>".to_string(),
            ),
            (
                "Import".to_string(),
                "prodex profile import-current".to_string(),
            ),
        ];
        print_panel("Profiles", &fields);
        return Ok(());
    }

    let summary_fields = vec![
        ("Count".to_string(), state.profiles.len().to_string()),
        (
            "Active".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Profiles", &summary_fields);

    for summary in collect_profile_summaries(&state) {
        let kind = if summary.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        let fields = vec![
            (
                "Current".to_string(),
                if summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            ("Auth".to_string(), summary.auth.label),
            (
                "Email".to_string(),
                summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            ("Path".to_string(), summary.codex_home.display().to_string()),
        ];
        print_panel(&format!("Profile {}", summary.name), &fields);
    }

    Ok(())
}

pub(crate) fn handle_remove_profile(args: RemoveProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    let Some(profile) = state.profiles.remove(&args.name) else {
        bail!("profile '{}' does not exist", args.name);
    };

    let should_delete_home = profile.managed || args.delete_home;
    if should_delete_home {
        if !profile.managed && args.delete_home {
            bail!(
                "refusing to delete external path {}",
                profile.codex_home.display()
            );
        }
        if profile.codex_home.exists() {
            fs::remove_dir_all(&profile.codex_home)
                .with_context(|| format!("failed to delete {}", profile.codex_home.display()))?;
        }
    }

    state.last_run_selected_at.remove(&args.name);
    state
        .response_profile_bindings
        .retain(|_, binding| binding.profile_name != args.name);
    state
        .session_profile_bindings
        .retain(|_, binding| binding.profile_name != args.name);

    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        state.active_profile = state.profiles.keys().next().cloned();
    }

    state.save(&paths)?;

    let mut fields = vec![(
        "Result".to_string(),
        format!("Removed profile '{}'.", args.name),
    )];
    fields.push((
        "Deleted home".to_string(),
        if args.delete_home {
            "Yes".to_string()
        } else {
            "No".to_string()
        },
    ));
    fields.push((
        "Active".to_string(),
        state
            .active_profile
            .clone()
            .unwrap_or_else(|| "cleared".to_string()),
    ));
    print_panel("Profile Removed", &fields);

    Ok(())
}

pub(crate) fn handle_set_active_profile(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let name = resolve_profile_name(&state, selector.profile.as_deref())?;
    state.active_profile = Some(name.clone());
    state.save(&paths)?;

    let profile = state
        .profiles
        .get(&name)
        .with_context(|| format!("profile '{}' disappeared from state", name))?;

    let fields = vec![
        ("Result".to_string(), format!("Active profile: {name}")),
        (
            "CODEX_HOME".to_string(),
            profile.codex_home.display().to_string(),
        ),
    ];
    print_panel("Active Profile", &fields);
    Ok(())
}

pub(crate) fn handle_current_profile() -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    let Some(active) = state.active_profile.as_deref() else {
        let mut fields = vec![("Status".to_string(), "No active profile.".to_string())];
        if state.profiles.len() == 1 {
            if let Some((name, profile)) = state.profiles.iter().next() {
                fields.push(("Only profile".to_string(), name.clone()));
                fields.push((
                    "CODEX_HOME".to_string(),
                    profile.codex_home.display().to_string(),
                ));
            }
        }
        print_panel("Active Profile", &fields);
        return Ok(());
    };

    let profile = state
        .profiles
        .get(active)
        .with_context(|| format!("active profile '{}' is missing", active))?;

    let fields = vec![
        ("Profile".to_string(), active.to_string()),
        (
            "CODEX_HOME".to_string(),
            profile.codex_home.display().to_string(),
        ),
        (
            "Managed".to_string(),
            if profile.managed {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Email".to_string(),
            profile.email.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Auth".to_string(),
            read_auth_summary(&profile.codex_home).label,
        ),
    ];
    print_panel("Active Profile", &fields);
    Ok(())
}

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
    let profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let codex_home = profile.codex_home.clone();
    let managed = profile.managed;

    if managed {
        prepare_managed_codex_home(paths, &codex_home)?;
    } else {
        create_codex_home_if_missing(&codex_home)?;
    }

    let status = run_codex_login(&codex_home, codex_args)?;
    if !status.success() {
        return Ok(status);
    }

    if let Ok(email) = fetch_profile_email(&codex_home) {
        if let Some(profile) = state.profiles.get_mut(&profile_name) {
            profile.email = Some(email);
        }
    }

    let account_email = state
        .profiles
        .get(&profile_name)
        .and_then(|profile| profile.email.clone())
        .unwrap_or_else(|| "-".to_string());
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;
    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in successfully for profile '{profile_name}'."),
        ),
        ("Account".to_string(), account_email),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(status)
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

    if let Some(profile_name) = find_profile_by_email(state, &email)? {
        let codex_home = state
            .profiles
            .get(&profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        let managed = codex_home.managed;
        let codex_home = codex_home.codex_home.clone();
        create_codex_home_if_missing(&codex_home)?;
        copy_directory_contents(&login_home, &codex_home)?;
        if managed {
            prepare_managed_codex_home(paths, &codex_home)?;
        }
        if let Some(profile) = state.profiles.get_mut(&profile_name) {
            profile.email = Some(email.clone());
        }
        remove_dir_if_exists(&login_home)?;
        state.active_profile = Some(profile_name.clone());
        state.save(paths)?;

        let fields = vec![
            (
                "Result".to_string(),
                format!("Logged in as {email}. Reusing profile '{profile_name}'."),
            ),
            ("Account".to_string(), email),
            ("Profile".to_string(), profile_name),
            ("CODEX_HOME".to_string(), codex_home.display().to_string()),
        ];
        print_panel("Login", &fields);
        return Ok(status);
    }

    let profile_name = unique_profile_name_for_email(paths, state, &email);
    let codex_home = absolutize(paths.managed_profiles_root.join(&profile_name))?;
    persist_login_home(&login_home, &codex_home)?;
    prepare_managed_codex_home(paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(email.clone()),
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in as {email}. Created profile '{profile_name}'."),
        ),
        ("Account".to_string(), email),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(status)
}

fn run_codex_login(codex_home: &Path, codex_args: &[OsString]) -> Result<ExitStatus> {
    let mut command_args = vec![OsString::from("login")];
    command_args.extend(codex_args.iter().cloned());
    run_child(&codex_bin(), &command_args, codex_home, &[], &[])
}

fn create_temporary_login_home(paths: &AppPaths) -> Result<PathBuf> {
    fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
        format!(
            "failed to create managed profile root {}",
            paths.managed_profiles_root.display()
        )
    })?;

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

pub(crate) fn handle_codex_logout(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, selector.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    let status = run_child(
        &codex_bin(),
        &[OsString::from("logout")],
        &codex_home,
        &[],
        &[],
    )?;
    exit_with_status(status)
}
