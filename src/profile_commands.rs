use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit},
};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::io::IsTerminal;

use super::profile_identity::{
    fetch_profile_email, find_profile_by_email, normalize_email, parse_email_from_auth_json,
    persist_login_home, remove_dir_if_exists, unique_profile_name_for_email,
};
use super::shared_codex_fs::{
    copy_codex_home, create_codex_home_if_missing, prepare_managed_codex_home,
};
use super::*;

const PROFILE_EXPORT_FORMAT: &str = "prodex_profile_export";
const PROFILE_EXPORT_VERSION: u32 = 1;
const PROFILE_EXPORT_CIPHER: &str = "aes_256_gcm_siv";
const PROFILE_EXPORT_KDF: &str = "pbkdf2_sha256";
const PROFILE_EXPORT_NONCE_BYTES: usize = 12;
const PROFILE_EXPORT_SALT_BYTES: usize = 16;
const PROFILE_EXPORT_KEY_BYTES: usize = 32;
const PROFILE_EXPORT_PBKDF2_ITERATIONS: u32 = if cfg!(test) { 1_000 } else { 600_000 };
const PROFILE_EXPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_EXPORT_PASSWORD";
const PROFILE_IMPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_IMPORT_PASSWORD";

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

pub(crate) fn handle_export_profiles(args: ExportProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_names = resolve_export_profile_names(&state, &args.profile)?;
    let payload = build_profile_export_payload(&state, &profile_names)?;
    let password = match resolve_export_password_mode(&args)? {
        true => Some(resolve_export_password()?),
        false => None,
    };
    let encoded = serialize_profile_export_payload(&payload, password.as_deref())?;
    let output_path = args
        .output
        .map(absolutize)
        .transpose()?
        .unwrap_or_else(default_profile_export_path);
    write_profile_export_bundle(&output_path, &encoded)?;
    audit_log_event_best_effort(
        "profile",
        "export",
        "success",
        serde_json::json!({
            "profile_count": profile_names.len(),
            "profile_names": profile_names,
            "encrypted": password.is_some(),
            "output_path": output_path.display().to_string(),
            "active_profile": payload.active_profile.clone(),
        }),
    );

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Exported {} profile(s).", profile_names.len()),
        ),
        ("Path".to_string(), output_path.display().to_string()),
        (
            "Encrypted".to_string(),
            if password.is_some() {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
    ];
    if payload.active_profile.is_some() {
        fields.push((
            "Active".to_string(),
            payload.active_profile.unwrap_or_default(),
        ));
    }
    print_panel("Profile Export", &fields);
    Ok(())
}

pub(crate) fn handle_import_profiles(args: ImportProfileArgs) -> Result<()> {
    let bundle_path = absolutize(args.path)?;
    let (payload, encrypted) = read_profile_export_payload(&bundle_path)?;
    let source_active_profile = payload.active_profile.clone();

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let commit = import_profile_export_payload(&paths, &mut state, &payload)?;
    if let Err(err) = state.save(&paths) {
        rollback_imported_profiles(&mut state, &commit);
        return Err(err);
    }
    audit_log_event_best_effort(
        "profile",
        "import",
        "success",
        serde_json::json!({
            "profile_count": payload.profiles.len(),
            "imported_profile_count": commit.imported_names.len(),
            "updated_existing_profile_count": commit.updated_existing_names.len(),
            "updated_existing_profile_names": commit.updated_existing_names.clone(),
            "bundle_path": bundle_path.display().to_string(),
            "encrypted": encrypted,
            "source_active_profile": source_active_profile.clone(),
            "active_profile": state.active_profile.clone(),
        }),
    );

    let result_message = match (
        commit.imported_names.len(),
        commit.updated_existing_names.len(),
    ) {
        (0, updated) => format!("Updated {updated} existing profile(s)."),
        (imported, 0) => format!("Imported {imported} profile(s)."),
        (imported, updated) => {
            format!("Imported {imported} profile(s) and updated {updated} existing profile(s).")
        }
    };
    let mut fields = vec![
        ("Result".to_string(), result_message),
        ("Path".to_string(), bundle_path.display().to_string()),
        (
            "Encrypted".to_string(),
            if encrypted {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Imported".to_string(),
            commit.imported_names.len().to_string(),
        ),
        (
            "Updated duplicates".to_string(),
            commit.updated_existing_names.len().to_string(),
        ),
    ];
    if let Some(active_profile) = source_active_profile {
        fields.push(("Source active".to_string(), active_profile));
    }
    if let Some(active_profile) = state.active_profile.clone() {
        fields.push(("Active".to_string(), active_profile));
    }
    print_panel("Profile Import", &fields);
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

    let target_names = if args.all {
        state.profiles.keys().cloned().collect::<Vec<_>>()
    } else {
        let Some(name) = args.name.as_deref() else {
            bail!("provide a profile name or pass --all");
        };
        if !state.profiles.contains_key(name) {
            bail!("profile '{}' does not exist", name);
        }
        vec![name.to_string()]
    };

    if args.all && args.delete_home {
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
    }

    let removed_names = target_names.iter().cloned().collect::<BTreeSet<_>>();
    let mut removed_profiles = Vec::with_capacity(target_names.len());
    for name in &target_names {
        let profile = state
            .profiles
            .remove(name)
            .with_context(|| format!("profile '{}' disappeared from state", name))?;

        let should_delete_home = profile.managed || args.delete_home;
        if should_delete_home {
            if !profile.managed && args.delete_home {
                bail!(
                    "refusing to delete external path {}",
                    profile.codex_home.display()
                );
            }
            if profile.codex_home.exists() {
                fs::remove_dir_all(&profile.codex_home).with_context(|| {
                    format!("failed to delete {}", profile.codex_home.display())
                })?;
            }
        }

        removed_profiles.push(RemovedProfileRecord {
            name: name.clone(),
            managed: profile.managed,
            deleted_home: should_delete_home,
            codex_home: profile.codex_home,
        });
    }

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

    state.save(&paths)?;
    persist_pruned_profile_runtime_sidecars(&paths, &state.profiles)?;

    if args.all {
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
        return Ok(());
    }

    let removed_profile = removed_profiles
        .into_iter()
        .next()
        .expect("single-profile removal should record the removed profile");
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

    if let Ok(email) = fetch_profile_email(&codex_home)
        && let Some(profile) = state.profiles.get_mut(&profile_name)
    {
        profile.email = Some(email);
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
    let auth_json = required_auth_json_text(&login_home)?;

    if let Some(profile_name) = find_profile_by_email(state, &email)? {
        let updated = update_existing_profile_auth(
            paths,
            state,
            &profile_name,
            Some(&email),
            &auth_json,
            true,
        )?;
        remove_dir_if_exists(&login_home)?;
        state.save(paths)?;

        let fields = vec![
            (
                "Result".to_string(),
                format!(
                    "Logged in as {email}. Updated auth token for existing profile '{}'.",
                    updated.profile_name
                ),
            ),
            ("Account".to_string(), email),
            ("Profile".to_string(), updated.profile_name),
            (
                "CODEX_HOME".to_string(),
                updated.codex_home.display().to_string(),
            ),
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

fn resolve_export_profile_names(state: &AppState, requested: &[String]) -> Result<Vec<String>> {
    if state.profiles.is_empty() {
        bail!("no profiles configured");
    }

    if requested.is_empty() {
        return Ok(state.profiles.keys().cloned().collect());
    }

    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in requested {
        if !seen.insert(name.clone()) {
            continue;
        }
        if !state.profiles.contains_key(name) {
            bail!("profile '{}' does not exist", name);
        }
        names.push(name.clone());
    }
    Ok(names)
}

fn build_profile_export_payload(
    state: &AppState,
    profile_names: &[String],
) -> Result<ProfileExportPayload> {
    let mut profiles = Vec::with_capacity(profile_names.len());
    for name in profile_names {
        let profile = state
            .profiles
            .get(name)
            .with_context(|| format!("profile '{}' is missing", name))?;
        let auth_path = secret_store::auth_json_path(&profile.codex_home);
        let auth_json = read_auth_json_text(&profile.codex_home)
            .with_context(|| format!("failed to read {}", auth_path.display()))?
            .with_context(|| format!("failed to read {}", auth_path.display()))?;
        let _: StoredAuth = serde_json::from_str(&auth_json)
            .with_context(|| format!("failed to parse {}", auth_path.display()))?;
        profiles.push(ExportedProfile {
            name: name.clone(),
            email: profile.email.clone(),
            source_managed: profile.managed,
            auth_json,
        });
    }

    Ok(ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: state
            .active_profile
            .clone()
            .filter(|active| profile_names.iter().any(|name| name == active)),
        profiles,
    })
}

fn default_profile_export_path() -> PathBuf {
    let file_name = format!(
        "prodex-profiles-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    );
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(file_name)
}

fn resolve_export_password_mode(args: &ExportProfileArgs) -> Result<bool> {
    if args.password_protect {
        return Ok(true);
    }
    if args.no_password {
        return Ok(false);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }
    prompt_yes_no("Password-protect export file? [y/N]: ", false)
}

fn resolve_export_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_EXPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "password protection requested but no interactive terminal is available; set {}",
            PROFILE_EXPORT_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password("Export password: ")
        .context("failed to read export password")?;
    if password.is_empty() {
        bail!("export password cannot be empty");
    }
    let confirmation = rpassword::prompt_password("Confirm export password: ")
        .context("failed to read export password confirmation")?;
    if password != confirmation {
        bail!("export passwords did not match");
    }
    Ok(password)
}

fn resolve_import_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_IMPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "profile export bundle is password-protected; set {} or rerun in a terminal",
            PROFILE_IMPORT_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password("Export password: ")
        .context("failed to read import password")?;
    if password.is_empty() {
        bail!("import password cannot be empty");
    }
    Ok(password)
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let mut input = String::new();
    loop {
        eprint!("{prompt}");
        io::stderr().flush().context("failed to flush prompt")?;
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read prompt response")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                eprintln!("Please answer yes or no.");
            }
        }
    }
}

fn serialize_profile_export_payload(
    payload: &ProfileExportPayload,
    password: Option<&str>,
) -> Result<Vec<u8>> {
    let envelope = match password {
        Some(password) => encrypt_profile_export_payload(payload, password)?,
        None => ProfileExportEnvelope::Plain {
            format: PROFILE_EXPORT_FORMAT.to_string(),
            version: PROFILE_EXPORT_VERSION,
            payload: payload.clone(),
        },
    };
    serde_json::to_vec_pretty(&envelope).context("failed to serialize profile export bundle")
}

fn encrypt_profile_export_payload(
    payload: &ProfileExportPayload,
    password: &str,
) -> Result<ProfileExportEnvelope> {
    let payload_json =
        serde_json::to_vec(payload).context("failed to serialize profile export payload")?;
    let mut salt = [0_u8; PROFILE_EXPORT_SALT_BYTES];
    getrandom::fill(&mut salt)
        .map_err(|err| anyhow::anyhow!("failed to generate export salt: {err}"))?;
    let mut nonce = [0_u8; PROFILE_EXPORT_NONCE_BYTES];
    getrandom::fill(&mut nonce)
        .map_err(|err| anyhow::anyhow!("failed to generate export nonce: {err}"))?;
    let key = derive_profile_export_key(password, &salt, PROFILE_EXPORT_PBKDF2_ITERATIONS);
    let cipher =
        Aes256GcmSiv::new_from_slice(&key).context("failed to initialize export cipher")?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), payload_json.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to encrypt profile export payload"))?;

    Ok(ProfileExportEnvelope::Encrypted {
        format: PROFILE_EXPORT_FORMAT.to_string(),
        version: PROFILE_EXPORT_VERSION,
        cipher: PROFILE_EXPORT_CIPHER.to_string(),
        kdf: PROFILE_EXPORT_KDF.to_string(),
        iterations: PROFILE_EXPORT_PBKDF2_ITERATIONS,
        salt_base64: base64::engine::general_purpose::STANDARD.encode(salt),
        nonce_base64: base64::engine::general_purpose::STANDARD.encode(nonce),
        ciphertext_base64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    })
}

fn derive_profile_export_key(
    password: &str,
    salt: &[u8],
    iterations: u32,
) -> [u8; PROFILE_EXPORT_KEY_BYTES] {
    let mut key = [0_u8; PROFILE_EXPORT_KEY_BYTES];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

fn read_profile_export_payload(path: &Path) -> Result<(ProfileExportPayload, bool)> {
    let content = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let envelope: ProfileExportEnvelope = serde_json::from_slice(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let encrypted = matches!(envelope, ProfileExportEnvelope::Encrypted { .. });
    let payload = decode_profile_export_envelope(envelope)?;
    Ok((payload, encrypted))
}

fn decode_profile_export_envelope(envelope: ProfileExportEnvelope) -> Result<ProfileExportPayload> {
    match envelope {
        ProfileExportEnvelope::Plain {
            format,
            version,
            payload,
        } => {
            validate_profile_export_header(&format, version)?;
            Ok(payload)
        }
        ProfileExportEnvelope::Encrypted {
            format,
            version,
            cipher,
            kdf,
            iterations,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            validate_profile_export_header(&format, version)?;
            if cipher != PROFILE_EXPORT_CIPHER {
                bail!("unsupported profile export cipher '{}'", cipher);
            }
            if kdf != PROFILE_EXPORT_KDF {
                bail!("unsupported profile export KDF '{}'", kdf);
            }
            let password = resolve_import_password()?;
            let salt = base64::engine::general_purpose::STANDARD
                .decode(salt_base64)
                .context("failed to decode encrypted export salt")?;
            let nonce = base64::engine::general_purpose::STANDARD
                .decode(nonce_base64)
                .context("failed to decode encrypted export nonce")?;
            let ciphertext = base64::engine::general_purpose::STANDARD
                .decode(ciphertext_base64)
                .context("failed to decode encrypted export payload")?;
            if nonce.len() != PROFILE_EXPORT_NONCE_BYTES {
                bail!("invalid encrypted export nonce length");
            }
            let key = derive_profile_export_key(&password, &salt, iterations);
            let cipher =
                Aes256GcmSiv::new_from_slice(&key).context("failed to initialize import cipher")?;
            let plaintext = cipher
                .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                .map_err(|_| anyhow::anyhow!("failed to decrypt profile export bundle"))?;
            serde_json::from_slice(&plaintext)
                .context("failed to parse decrypted profile export payload")
        }
    }
}

fn validate_profile_export_header(format: &str, version: u32) -> Result<()> {
    if format != PROFILE_EXPORT_FORMAT {
        bail!("unsupported profile export format '{}'", format);
    }
    if version != PROFILE_EXPORT_VERSION {
        bail!(
            "unsupported profile export version {} (expected {})",
            version,
            PROFILE_EXPORT_VERSION
        );
    }
    Ok(())
}

fn write_profile_export_bundle(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temp_path = unique_state_temp_file_path(path);
    fs::write(&temp_path, content)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to secure {}", path.display()))?;
    }
    Ok(())
}

fn import_profile_export_payload(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<ImportedProfilesCommit> {
    let prepared = stage_imported_profiles(paths, state, payload)?;
    let previous_active_profile = state.active_profile.clone();
    let mut committed_homes = Vec::with_capacity(prepared.staged_profiles.len());
    let mut imported_names = Vec::with_capacity(prepared.staged_profiles.len());
    let mut updated_existing_names = Vec::with_capacity(prepared.auth_updates.len());
    let mut auth_updates = Vec::with_capacity(prepared.auth_updates.len());

    let result = (|| -> Result<()> {
        for update in &prepared.auth_updates {
            let previous = state
                .profiles
                .get(&update.target_profile_name)
                .with_context(|| format!("profile '{}' is missing", update.target_profile_name))?
                .clone();
            let previous_auth_json =
                read_auth_json_text(&previous.codex_home).with_context(|| {
                    format!(
                        "failed to read {}",
                        secret_store::auth_json_path(&previous.codex_home).display()
                    )
                })?;
            let updated = update_existing_profile_auth(
                paths,
                state,
                &update.target_profile_name,
                update.email.as_deref(),
                &update.auth_json,
                false,
            )?;
            updated_existing_names.push(updated.profile_name.clone());
            auth_updates.push(ImportedExistingProfileAuthUpdate {
                profile_name: updated.profile_name,
                codex_home: updated.codex_home,
                previous_auth_json,
                previous_email: previous.email,
            });
        }

        for staged in &prepared.staged_profiles {
            fs::rename(&staged.staging_home, &staged.final_home).with_context(|| {
                format!(
                    "failed to finalize imported profile home {}",
                    staged.final_home.display()
                )
            })?;
            committed_homes.push(staged.final_home.clone());
            imported_names.push(staged.name.clone());
            state.profiles.insert(
                staged.name.clone(),
                ProfileEntry {
                    codex_home: staged.final_home.clone(),
                    managed: true,
                    email: staged.email.clone(),
                },
            );
        }

        if state.active_profile.is_none()
            && let Some(active_profile) = payload.active_profile.as_ref()
            && let Some(resolved_profile_name) = prepared.resolved_profile_names.get(active_profile)
        {
            state.active_profile = Some(resolved_profile_name.clone());
        }
        Ok(())
    })();

    if let Err(err) = result {
        for name in &imported_names {
            state.profiles.remove(name);
        }
        for update in auth_updates.iter().rev() {
            if let Some(profile) = state.profiles.get_mut(&update.profile_name) {
                profile.email = update.previous_email.clone();
            }
            if let Some(previous_auth_json) = update.previous_auth_json.as_deref() {
                let _ = write_secret_text_file(
                    &secret_store::auth_json_path(&update.codex_home),
                    previous_auth_json,
                );
            } else {
                let _ = fs::remove_file(secret_store::auth_json_path(&update.codex_home));
            }
        }
        state.active_profile = previous_active_profile.clone();
        for home in committed_homes.iter().rev() {
            let _ = fs::remove_dir_all(home);
        }
        return Err(err);
    }

    Ok(ImportedProfilesCommit {
        imported_names,
        updated_existing_names,
        committed_homes,
        auth_updates,
        previous_active_profile,
    })
}

fn rollback_imported_profiles(state: &mut AppState, commit: &ImportedProfilesCommit) {
    for name in &commit.imported_names {
        state.profiles.remove(name);
        state.last_run_selected_at.remove(name);
        state
            .response_profile_bindings
            .retain(|_, binding| binding.profile_name != *name);
        state
            .session_profile_bindings
            .retain(|_, binding| binding.profile_name != *name);
    }
    for update in commit.auth_updates.iter().rev() {
        if let Some(profile) = state.profiles.get_mut(&update.profile_name) {
            profile.email = update.previous_email.clone();
        }
        if let Some(previous_auth_json) = update.previous_auth_json.as_deref() {
            let _ = write_secret_text_file(
                &secret_store::auth_json_path(&update.codex_home),
                previous_auth_json,
            );
        } else {
            let _ = fs::remove_file(secret_store::auth_json_path(&update.codex_home));
        }
    }
    state.active_profile = commit.previous_active_profile.clone();
    for home in commit.committed_homes.iter().rev() {
        let _ = fs::remove_dir_all(home);
    }
}

fn stage_imported_profiles(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<PreparedImportedProfiles> {
    if payload.profiles.is_empty() {
        bail!("profile export bundle does not contain any profiles");
    }

    fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
        format!(
            "failed to create managed profile root {}",
            paths.managed_profiles_root.display()
        )
    })?;

    let mut seen_names = BTreeSet::new();
    let mut staged_profiles = Vec::with_capacity(payload.profiles.len());
    let mut auth_updates = Vec::new();
    let mut resolved_profile_names = BTreeMap::new();
    let mut email_targets = BTreeMap::new();
    let result = (|| -> Result<()> {
        for exported in &payload.profiles {
            validate_profile_name(&exported.name)?;
            if !seen_names.insert(exported.name.clone()) {
                bail!(
                    "profile export bundle contains duplicate profile '{}'",
                    exported.name
                );
            }

            let _: StoredAuth = serde_json::from_str(&exported.auth_json).with_context(|| {
                format!(
                    "failed to parse exported auth.json for profile '{}'",
                    exported.name
                )
            })?;
            let resolved_email = resolved_exported_profile_email(exported);

            if let Some(email) = resolved_email.as_deref() {
                let normalized_email = normalize_email(email);
                if let Some(target) = email_targets.get(&normalized_email) {
                    match target {
                        ImportEmailTarget::Existing(profile_name) => {
                            queue_existing_profile_auth_update(
                                &mut auth_updates,
                                profile_name,
                                resolved_email.clone(),
                                exported.auth_json.clone(),
                            );
                            resolved_profile_names
                                .insert(exported.name.clone(), profile_name.clone());
                            continue;
                        }
                        ImportEmailTarget::PendingNew(index) => {
                            let staged: &mut StagedImportedProfile =
                                staged_profiles.get_mut(*index).with_context(|| {
                                    format!(
                                        "staged import profile index {} is missing for '{}'",
                                        index, exported.name
                                    )
                                })?;
                            write_secret_text_file(
                                &staged.staging_home.join("auth.json"),
                                &exported.auth_json,
                            )?;
                            staged.email = resolved_email.clone();
                            resolved_profile_names
                                .insert(exported.name.clone(), staged.name.clone());
                            continue;
                        }
                    }
                }

                if let Some(existing_profile_name) = find_profile_by_email(state, email)? {
                    email_targets.insert(
                        normalized_email,
                        ImportEmailTarget::Existing(existing_profile_name.clone()),
                    );
                    queue_existing_profile_auth_update(
                        &mut auth_updates,
                        &existing_profile_name,
                        resolved_email.clone(),
                        exported.auth_json.clone(),
                    );
                    resolved_profile_names.insert(exported.name.clone(), existing_profile_name);
                    continue;
                }
            }

            if state.profiles.contains_key(&exported.name) {
                bail!("profile '{}' already exists", exported.name);
            }

            let final_home = absolutize(paths.managed_profiles_root.join(&exported.name))?;
            ensure_path_is_unique(state, &final_home)?;
            if final_home.exists() {
                bail!(
                    "managed profile home {} already exists",
                    final_home.display()
                );
            }

            let staging_home = unique_import_staging_home(paths, &exported.name);
            create_codex_home_if_missing(&staging_home)?;
            prepare_managed_codex_home(paths, &staging_home)?;
            write_secret_text_file(&staging_home.join("auth.json"), &exported.auth_json)?;

            let new_index = staged_profiles.len();
            staged_profiles.push(StagedImportedProfile {
                name: exported.name.clone(),
                email: resolved_email.clone(),
                staging_home,
                final_home,
            });
            resolved_profile_names.insert(exported.name.clone(), exported.name.clone());
            if let Some(email) = resolved_email {
                email_targets.insert(
                    normalize_email(&email),
                    ImportEmailTarget::PendingNew(new_index),
                );
            }
        }
        Ok(())
    })();

    if let Err(err) = result {
        for staged in &staged_profiles {
            let _ = fs::remove_dir_all(&staged.staging_home);
        }
        return Err(err);
    }

    Ok(PreparedImportedProfiles {
        staged_profiles,
        auth_updates,
        resolved_profile_names,
    })
}

fn unique_import_staging_home(paths: &AppPaths, profile_name: &str) -> PathBuf {
    paths.managed_profiles_root.join(format!(
        ".import-{}-{}",
        profile_name,
        runtime_random_token("profile")
    ))
}

fn write_secret_text_file(path: &Path, content: &str) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), content)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
#[path = "../tests/support/profile_commands_internal_harness.rs"]
mod profile_commands_internal_tests;
