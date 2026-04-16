use super::profile_identity::{
    fetch_profile_email, find_profile_by_email, normalize_email, parse_email_from_auth_json,
    persist_login_home, remove_dir_if_exists, unique_profile_name_for_email,
};
use super::shared_codex_fs::{
    copy_codex_home, create_codex_home_if_missing, prepare_managed_codex_home,
};
use super::*;

mod import_export;

#[cfg(test)]
use self::import_export::{
    PROFILE_EXPORT_CIPHER, PROFILE_EXPORT_KDF, build_profile_export_payload,
    decode_profile_export_envelope, derive_profile_export_key, import_profile_export_payload,
    serialize_profile_export_payload, stage_imported_profiles, validate_profile_export_header,
};
pub(crate) use self::import_export::{
    handle_export_profiles, handle_import_current_profile, handle_import_profiles,
};
use self::import_export::{
    remove_committed_import_homes, rollback_imported_auth_updates, write_secret_text_file,
};
#[cfg(test)]
use aes_gcm_siv::aead::Aead;
#[cfg(test)]
use aes_gcm_siv::aead::KeyInit;
#[cfg(test)]
use aes_gcm_siv::{Aes256GcmSiv, Nonce};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileExportPayload {
    exported_at: String,
    source_prodex_version: String,
    active_profile: Option<String>,
    profiles: Vec<ExportedProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportedProfile {
    name: String,
    #[serde(default)]
    email: Option<String>,
    source_managed: bool,
    auth_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "payload_kind", rename_all = "snake_case")]
enum ProfileExportEnvelope {
    Plain {
        format: String,
        version: u32,
        payload: ProfileExportPayload,
    },
    Encrypted {
        format: String,
        version: u32,
        cipher: String,
        kdf: String,
        iterations: u32,
        salt_base64: String,
        nonce_base64: String,
        ciphertext_base64: String,
    },
}

#[derive(Debug)]
struct StagedImportedProfile {
    name: String,
    email: Option<String>,
    staging_home: PathBuf,
    final_home: PathBuf,
}

#[derive(Debug)]
struct PreparedImportedProfiles {
    staged_profiles: Vec<StagedImportedProfile>,
    auth_updates: Vec<PreparedImportedProfileAuthUpdate>,
    resolved_profile_names: BTreeMap<String, String>,
}

#[derive(Debug)]
struct PreparedImportedProfileAuthUpdate {
    target_profile_name: String,
    email: Option<String>,
    auth_json: String,
}

#[derive(Debug)]
struct ImportedExistingProfileAuthUpdate {
    profile_name: String,
    codex_home: PathBuf,
    previous_auth_json: Option<String>,
    previous_email: Option<String>,
}

#[derive(Debug)]
struct ImportedProfilesCommit {
    imported_names: Vec<String>,
    updated_existing_names: Vec<String>,
    committed_homes: Vec<PathBuf>,
    auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    previous_active_profile: Option<String>,
}

#[derive(Debug)]
struct ImportedProfilesTransaction {
    imported_names: Vec<String>,
    updated_existing_names: Vec<String>,
    committed_homes: Vec<PathBuf>,
    auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    previous_active_profile: Option<String>,
}

impl ImportedProfilesTransaction {
    fn new(
        previous_active_profile: Option<String>,
        staged_profile_count: usize,
        auth_update_count: usize,
    ) -> Self {
        Self {
            imported_names: Vec::with_capacity(staged_profile_count),
            updated_existing_names: Vec::with_capacity(auth_update_count),
            committed_homes: Vec::with_capacity(staged_profile_count),
            auth_updates: Vec::with_capacity(auth_update_count),
            previous_active_profile,
        }
    }

    fn record_existing_auth_update(&mut self, update: ImportedExistingProfileAuthUpdate) {
        self.updated_existing_names
            .push(update.profile_name.clone());
        self.auth_updates.push(update);
    }

    fn record_imported_profile(&mut self, name: String, final_home: PathBuf) {
        self.committed_homes.push(final_home);
        self.imported_names.push(name);
    }

    fn rollback_partial(&self, state: &mut AppState) {
        for name in &self.imported_names {
            state.profiles.remove(name);
        }
        rollback_imported_auth_updates(state, &self.auth_updates);
        state.active_profile = self.previous_active_profile.clone();
        remove_committed_import_homes(&self.committed_homes);
    }

    fn into_commit(self) -> ImportedProfilesCommit {
        ImportedProfilesCommit {
            imported_names: self.imported_names,
            updated_existing_names: self.updated_existing_names,
            committed_homes: self.committed_homes,
            auth_updates: self.auth_updates,
            previous_active_profile: self.previous_active_profile,
        }
    }
}

#[derive(Debug)]
enum ImportEmailTarget {
    Existing(String),
    PendingNew(usize),
}

#[derive(Debug)]
struct ExistingProfileAuthUpdate {
    profile_name: String,
    codex_home: PathBuf,
}

fn required_auth_json_text(codex_home: &Path) -> Result<String> {
    let auth_path = secret_store::auth_json_path(codex_home);
    read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_path.display()))?
        .with_context(|| format!("failed to read {}", auth_path.display()))
}

fn update_existing_profile_auth(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    email: Option<&str>,
    auth_json: &str,
    activate: bool,
) -> Result<ExistingProfileAuthUpdate> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .clone();

    if profile.managed {
        prepare_managed_codex_home(paths, &profile.codex_home)?;
    } else {
        create_codex_home_if_missing(&profile.codex_home)?;
    }
    write_secret_text_file(
        &secret_store::auth_json_path(&profile.codex_home),
        auth_json,
    )?;

    if let Some(email) = email
        && let Some(profile_entry) = state.profiles.get_mut(profile_name)
    {
        profile_entry.email = Some(email.to_string());
    }
    if activate {
        state.active_profile = Some(profile_name.to_string());
    }

    Ok(ExistingProfileAuthUpdate {
        profile_name: profile_name.to_string(),
        codex_home: profile.codex_home,
    })
}

fn resolved_exported_profile_email(exported: &ExportedProfile) -> Option<String> {
    parse_email_from_auth_json(&exported.auth_json)
        .ok()
        .flatten()
        .or_else(|| {
            exported
                .email
                .as_deref()
                .map(str::trim)
                .filter(|email| !email.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn queue_existing_profile_auth_update(
    auth_updates: &mut Vec<PreparedImportedProfileAuthUpdate>,
    target_profile_name: &str,
    email: Option<String>,
    auth_json: String,
) {
    if let Some(existing) = auth_updates
        .iter_mut()
        .find(|update| update.target_profile_name == target_profile_name)
    {
        existing.auth_json = auth_json;
        if email.is_some() {
            existing.email = email;
        }
        return;
    }

    auth_updates.push(PreparedImportedProfileAuthUpdate {
        target_profile_name: target_profile_name.to_string(),
        email,
        auth_json,
    });
}

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
            email: source_email,
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

#[derive(Debug)]
struct RemovedProfileRecord {
    name: String,
    managed: bool,
    deleted_home: bool,
    codex_home: PathBuf,
}

fn persist_pruned_profile_runtime_sidecars(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    let continuations_exist = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    if continuations_exist {
        let continuations = load_runtime_continuations_with_recovery(paths, profiles)?.value;
        save_runtime_continuations_for_profiles(paths, &continuations, profiles)?;
    }

    let journal_exists = runtime_continuation_journal_file_path(paths).exists()
        || runtime_continuation_journal_last_good_file_path(paths).exists();
    if journal_exists {
        let journal = load_runtime_continuation_journal_with_recovery(paths, profiles)?.value;
        save_runtime_continuation_journal_for_profiles(
            paths,
            &journal.continuations,
            profiles,
            journal.saved_at,
        )?;
    }

    Ok(())
}

pub(crate) fn handle_remove_profile(args: RemoveProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    let target_names = resolve_remove_profile_targets(&state, &args)?;
    validate_bulk_profile_home_deletion(&state, &target_names, &args)?;
    let removed_profiles = remove_profiles_from_state(&mut state, &target_names, args.delete_home)?;
    prune_removed_profile_metadata(&mut state, &target_names);
    state.save(&paths)?;
    persist_pruned_profile_runtime_sidecars(&paths, &state.profiles)?;

    if args.all {
        print_bulk_profile_removal_result(&state, &removed_profiles);
        return Ok(());
    }

    let removed_profile = removed_profiles
        .into_iter()
        .next()
        .expect("single-profile removal should record the removed profile");
    print_single_profile_removal_result(&state, removed_profile);

    Ok(())
}

fn resolve_remove_profile_targets(
    state: &AppState,
    args: &RemoveProfileArgs,
) -> Result<Vec<String>> {
    if args.all {
        return Ok(state.profiles.keys().cloned().collect::<Vec<_>>());
    }

    let Some(name) = args.name.as_deref() else {
        bail!("provide a profile name or pass --all");
    };
    if !state.profiles.contains_key(name) {
        bail!("profile '{}' does not exist", name);
    }
    Ok(vec![name.to_string()])
}

fn validate_bulk_profile_home_deletion(
    state: &AppState,
    target_names: &[String],
    args: &RemoveProfileArgs,
) -> Result<()> {
    if !args.all || !args.delete_home {
        return Ok(());
    }

    let external_profiles = target_names
        .iter()
        .filter(|name| {
            state
                .profiles
                .get(*name)
                .is_some_and(|profile| !profile.managed)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !external_profiles.is_empty() {
        bail!(
            "--delete-home with --all refuses to delete external profiles: {}",
            external_profiles.join(", ")
        );
    }

    Ok(())
}

fn remove_profiles_from_state(
    state: &mut AppState,
    target_names: &[String],
    delete_home: bool,
) -> Result<Vec<RemovedProfileRecord>> {
    let mut removed_profiles = Vec::with_capacity(target_names.len());
    for name in target_names {
        let profile = state
            .profiles
            .remove(name)
            .with_context(|| format!("profile '{}' disappeared from state", name))?;
        let deleted_home = remove_profile_home_if_requested(&profile, delete_home)?;
        removed_profiles.push(RemovedProfileRecord {
            name: name.clone(),
            managed: profile.managed,
            deleted_home,
            codex_home: profile.codex_home,
        });
    }

    Ok(removed_profiles)
}

fn remove_profile_home_if_requested(profile: &ProfileEntry, delete_home: bool) -> Result<bool> {
    let should_delete_home = profile.managed || delete_home;
    if !should_delete_home {
        return Ok(false);
    }

    if !profile.managed && delete_home {
        bail!(
            "refusing to delete external path {}",
            profile.codex_home.display()
        );
    }
    if profile.codex_home.exists() {
        fs::remove_dir_all(&profile.codex_home)
            .with_context(|| format!("failed to delete {}", profile.codex_home.display()))?;
    }

    Ok(true)
}

fn prune_removed_profile_metadata(state: &mut AppState, target_names: &[String]) {
    let removed_names = target_names.iter().cloned().collect::<BTreeSet<_>>();
    state
        .last_run_selected_at
        .retain(|profile_name, _| !removed_names.contains(profile_name));
    state
        .response_profile_bindings
        .retain(|_, binding| !removed_names.contains(&binding.profile_name));
    state
        .session_profile_bindings
        .retain(|_, binding| !removed_names.contains(&binding.profile_name));

    if state
        .active_profile
        .as_deref()
        .is_some_and(|profile_name| removed_names.contains(profile_name))
    {
        state.active_profile = state.profiles.keys().next().cloned();
    }
}

fn print_bulk_profile_removal_result(state: &AppState, removed_profiles: &[RemovedProfileRecord]) {
    audit_log_event_best_effort(
        "profile",
        "remove",
        "success",
        serde_json::json!({
            "all": true,
            "removed_count": removed_profiles.len(),
            "profile_names": removed_profiles.iter().map(|profile| profile.name.clone()).collect::<Vec<_>>(),
            "deleted_home_count": removed_profiles.iter().filter(|profile| profile.deleted_home).count(),
            "active_profile": state.active_profile.clone(),
        }),
    );

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Removed {} profile(s).", removed_profiles.len()),
        ),
        (
            "Deleted homes".to_string(),
            removed_profiles
                .iter()
                .filter(|profile| profile.deleted_home)
                .count()
                .to_string(),
        ),
        (
            "Active".to_string(),
            state
                .active_profile
                .clone()
                .unwrap_or_else(|| "cleared".to_string()),
        ),
    ];
    if !removed_profiles.is_empty() {
        fields.push((
            "Profiles".to_string(),
            removed_profiles
                .iter()
                .map(|profile| profile.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    print_panel("Profiles Removed", &fields);
}

fn print_single_profile_removal_result(state: &AppState, removed_profile: RemovedProfileRecord) {
    audit_log_event_best_effort(
        "profile",
        "remove",
        "success",
        serde_json::json!({
            "profile_name": removed_profile.name.clone(),
            "managed": removed_profile.managed,
            "deleted_home": removed_profile.deleted_home,
            "codex_home": removed_profile.codex_home.display().to_string(),
            "active_profile": state.active_profile.clone(),
        }),
    );

    let fields = vec![
        (
            "Result".to_string(),
            format!("Removed profile '{}'.", removed_profile.name),
        ),
        (
            "Deleted home".to_string(),
            if removed_profile.deleted_home {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Active".to_string(),
            state
                .active_profile
                .clone()
                .unwrap_or_else(|| "cleared".to_string()),
        ),
    ];
    print_panel("Profile Removed", &fields);
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
    let codex_home = absolutize(paths.managed_profiles_root.join(&profile_name))?;
    persist_login_home(login_home, &codex_home)?;
    prepare_managed_codex_home(paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(email.to_string()),
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

pub(crate) fn handle_codex_logout(args: LogoutArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.selected_profile())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    let status = run_child_plan(
        &ChildProcessPlan::new(codex_bin(), codex_home.clone())
            .with_args(vec![OsString::from("logout")]),
        None,
    )?;
    exit_with_status(status)
}

#[cfg(test)]
#[path = "../tests/support/profile_commands_internal_harness.rs"]
mod profile_commands_internal_tests;
