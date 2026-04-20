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
    let activate_profile = state.active_profile.is_none() || args.activate;
    let source_email = source_home
        .as_deref()
        .and_then(|home| fetch_profile_email(home).ok());

    if let Some(source) = source_home.as_deref()
        && let Some(email) = source_email.as_deref()
        && let Some(profile_name) = find_profile_by_email(&mut state, email)?
        && let Ok(Some(auth_json)) = read_auth_json_text(source)
    {
        let updated = update_existing_profile_auth(
            &paths,
            &mut state,
            &profile_name,
            Some(email),
            &auth_json,
            activate_profile,
        )?;
        let updated_profile_name = updated.profile_name.clone();
        let updated_codex_home = updated.codex_home.clone();
        state.save(&paths)?;
        audit_log_event_best_effort(
            "profile",
            "add",
            "success",
            serde_json::json!({
                "profile_name": updated_profile_name.clone(),
                "requested_name": args.name.clone(),
                "duplicate_email": true,
                "email": email,
                "updated_token_only": true,
                "source_home": source.display().to_string(),
                "codex_home": updated_codex_home.display().to_string(),
                "activated": state.active_profile.as_deref() == Some(updated_profile_name.as_str()),
            }),
        );

        let mut fields = vec![
            (
                "Result".to_string(),
                format!(
                    "Detected duplicate account {email}. Updated auth token for profile '{}'.",
                    updated_profile_name
                ),
            ),
            ("Account".to_string(), email.to_string()),
            ("Profile".to_string(), updated.profile_name.clone()),
            (
                "CODEX_HOME".to_string(),
                updated_codex_home.display().to_string(),
            ),
            (
                "Storage".to_string(),
                "Existing profile token updated.".to_string(),
            ),
        ];
        if state.active_profile.as_deref() == Some(updated.profile_name.as_str()) {
            fields.push(("Active".to_string(), updated.profile_name));
        }
        print_panel("Profile Updated", &fields);
        return Ok(());
    }

    let codex_home = match args.codex_home {
        Some(path) => {
            let home = absolutize(path)?;
            create_codex_home_if_missing(&home)?;
            home
        }
        None => {
            let home = managed_profile_home_path(&paths, &args.name)?;
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
            email: source_email,
            provider: ProfileProvider::Openai,
        },
    );

    if activate_profile {
        state.active_profile = Some(args.name.clone());
    }

    state.save(&paths)?;
    audit_log_event_best_effort(
        "profile",
        "add",
        "success",
        serde_json::json!({
            "profile_name": args.name.clone(),
            "managed": managed,
            "activated": state.active_profile.as_deref() == Some(args.name.as_str()),
            "copied_source": source_home.is_some(),
            "codex_home": codex_home.display().to_string(),
            "source_home": source_home.as_ref().map(|path| path.display().to_string()),
        }),
    );

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
            (
                "Import Copilot".to_string(),
                "prodex profile import copilot".to_string(),
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
            (
                "Provider".to_string(),
                summary.provider.display_name().to_string(),
            ),
            ("Auth".to_string(), summary.auth.label),
            (
                "Identity".to_string(),
                summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            ("Path".to_string(), summary.codex_home.display().to_string()),
        ];
        print_panel(&format!("Profile {}", summary.name), &fields);
    }

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
    audit_log_event_best_effort(
        "profile",
        "set_active",
        "success",
        serde_json::json!({
            "profile_name": name.clone(),
            "codex_home": profile.codex_home.display().to_string(),
        }),
    );

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
        if state.profiles.len() == 1
            && let Some((name, profile)) = state.profiles.iter().next()
        {
            fields.push(("Only profile".to_string(), name.clone()));
            fields.push((
                "CODEX_HOME".to_string(),
                profile.codex_home.display().to_string(),
            ));
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
            "Provider".to_string(),
            profile.provider.display_name().to_string(),
        ),
        (
            "Identity".to_string(),
            profile.email.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Auth".to_string(),
            profile.provider.auth_summary(&profile.codex_home).label,
        ),
    ];
    print_panel("Active Profile", &fields);
    Ok(())
}
