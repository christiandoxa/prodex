use anyhow::{Context, Result, bail};
use chrono::{Local, TimeZone};
use clap::{Args, Parser, Subcommand};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

const DEFAULT_PRODEX_DIR: &str = ".prodex";
const DEFAULT_CODEX_DIR: &str = ".codex";

#[derive(Parser, Debug)]
#[command(
    name = "prodex",
    version,
    about = "Manage multiple Codex profiles backed by isolated CODEX_HOME directories."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(subcommand)]
    Profile(ProfileCommands),
    #[command(name = "use")]
    UseProfile(ProfileSelector),
    Current,
    Doctor(DoctorArgs),
    #[command(trailing_var_arg = true)]
    Login(CodexPassthroughArgs),
    Logout(ProfileSelector),
    Quota(QuotaArgs),
    #[command(trailing_var_arg = true)]
    Run(RunArgs),
}

#[derive(Subcommand, Debug)]
enum ProfileCommands {
    Add(AddProfileArgs),
    ImportCurrent(ImportCurrentArgs),
    List,
    Remove(RemoveProfileArgs),
    Use(ProfileSelector),
}

#[derive(Args, Debug)]
struct AddProfileArgs {
    name: String,
    #[arg(long)]
    codex_home: Option<PathBuf>,
    #[arg(long)]
    copy_from: Option<PathBuf>,
    #[arg(long)]
    copy_current: bool,
    #[arg(long)]
    activate: bool,
}

#[derive(Args, Debug)]
struct ImportCurrentArgs {
    #[arg(default_value = "default")]
    name: String,
}

#[derive(Args, Debug)]
struct RemoveProfileArgs {
    name: String,
    #[arg(long)]
    delete_home: bool,
}

#[derive(Args, Debug, Clone)]
struct ProfileSelector {
    #[arg(short, long)]
    profile: Option<String>,
}

#[derive(Args, Debug)]
struct CodexPassthroughArgs {
    #[arg(short, long)]
    profile: Option<String>,
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
struct QuotaArgs {
    #[arg(short, long)]
    profile: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    raw: bool,
    #[arg(long)]
    watch: bool,
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Args, Debug)]
struct DoctorArgs {
    #[arg(long)]
    quota: bool,
}

#[derive(Args, Debug)]
struct RunArgs {
    #[arg(short, long)]
    profile: Option<String>,
    #[arg(long)]
    skip_quota_check: bool,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    codex_args: Vec<OsString>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppState {
    active_profile: Option<String>,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileEntry {
    codex_home: PathBuf,
    managed: bool,
}

#[derive(Debug, Clone)]
struct AppPaths {
    root: PathBuf,
    state_file: PathBuf,
    managed_profiles_root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageResponse {
    email: Option<String>,
    plan_type: Option<String>,
    rate_limit: Option<WindowPair>,
    code_review_rate_limit: Option<WindowPair>,
    #[serde(default)]
    additional_rate_limits: Vec<AdditionalRateLimit>,
}

#[derive(Debug, Clone, Deserialize)]
struct WindowPair {
    primary_window: Option<UsageWindow>,
    secondary_window: Option<UsageWindow>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdditionalRateLimit {
    limit_name: Option<String>,
    metered_feature: Option<String>,
    rate_limit: WindowPair,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageWindow {
    used_percent: Option<i64>,
    reset_at: Option<i64>,
    limit_window_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredAuth {
    auth_mode: Option<String>,
    tokens: Option<StoredTokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredTokens {
    access_token: Option<String>,
}

#[derive(Debug)]
struct BlockedLimit {
    label: String,
    reset_at: Option<i64>,
}

#[derive(Debug, Clone)]
struct AuthSummary {
    label: String,
    quota_compatible: bool,
}

#[derive(Debug)]
struct QuotaReport {
    name: String,
    active: bool,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Profile(command) => handle_profile_command(command),
        Commands::UseProfile(selector) => handle_set_active_profile(selector),
        Commands::Current => handle_current_profile(),
        Commands::Doctor(args) => handle_doctor(args),
        Commands::Login(args) => handle_codex_login(args),
        Commands::Logout(selector) => handle_codex_logout(selector),
        Commands::Quota(args) => handle_quota(args),
        Commands::Run(args) => handle_run(args),
    }
}

fn handle_profile_command(command: ProfileCommands) -> Result<()> {
    match command {
        ProfileCommands::Add(args) => handle_add_profile(args),
        ProfileCommands::ImportCurrent(args) => handle_import_current_profile(args),
        ProfileCommands::List => handle_list_profiles(),
        ProfileCommands::Remove(args) => handle_remove_profile(args),
        ProfileCommands::Use(selector) => handle_set_active_profile(selector),
    }
}

fn handle_add_profile(args: AddProfileArgs) -> Result<()> {
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
        Some(default_codex_home()?)
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

    ensure_path_is_unique(&state, &codex_home)?;

    state.profiles.insert(
        args.name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed,
        },
    );

    if state.active_profile.is_none() || args.activate {
        state.active_profile = Some(args.name.clone());
    }

    state.save(&paths)?;

    println!("Added profile '{}'.", args.name);
    println!("CODEX_HOME: {}", codex_home.display());
    if source_home.is_some() {
        println!("Source copied into managed profile home.");
    } else if managed {
        println!("Managed profile home created.");
    } else {
        println!("Existing CODEX_HOME registered.");
    }
    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        println!("Active profile: {}", args.name);
    }

    Ok(())
}

fn handle_import_current_profile(args: ImportCurrentArgs) -> Result<()> {
    handle_add_profile(AddProfileArgs {
        name: args.name,
        codex_home: None,
        copy_from: None,
        copy_current: true,
        activate: true,
    })
}

fn handle_list_profiles() -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if state.profiles.is_empty() {
        println!("No profiles configured.");
        println!("Create one with: prodex profile add <name>");
        println!("To import the current Codex home: prodex profile import-current");
        return Ok(());
    }

    for (name, profile) in &state.profiles {
        let active = if state.active_profile.as_deref() == Some(name.as_str()) {
            "*"
        } else {
            " "
        };
        let auth_state = read_auth_summary(&profile.codex_home);
        let kind = if profile.managed {
            "managed"
        } else {
            "external"
        };

        println!("{active} {name}");
        println!("  kind: {kind}");
        println!("  auth: {}", auth_state.label);
        println!("  path: {}", profile.codex_home.display());
    }

    Ok(())
}

fn handle_remove_profile(args: RemoveProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    let Some(profile) = state.profiles.remove(&args.name) else {
        bail!("profile '{}' does not exist", args.name);
    };

    if args.delete_home {
        if !profile.managed {
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

    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        state.active_profile = state.profiles.keys().next().cloned();
    }

    state.save(&paths)?;

    println!("Removed profile '{}'.", args.name);
    if args.delete_home {
        println!("Deleted managed profile home.");
    }
    match &state.active_profile {
        Some(active) => println!("Active profile: {active}"),
        None => println!("Active profile cleared."),
    }

    Ok(())
}

fn handle_set_active_profile(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let name = resolve_profile_name(&state, selector.profile.as_deref())?;
    state.active_profile = Some(name.clone());
    state.save(&paths)?;

    let profile = state
        .profiles
        .get(&name)
        .with_context(|| format!("profile '{}' disappeared from state", name))?;

    println!("Active profile: {name}");
    println!("CODEX_HOME: {}", profile.codex_home.display());
    Ok(())
}

fn handle_current_profile() -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    let Some(active) = state.active_profile.as_deref() else {
        println!("No active profile.");
        if state.profiles.len() == 1 {
            if let Some((name, profile)) = state.profiles.iter().next() {
                println!("Only configured profile: {name}");
                println!("CODEX_HOME: {}", profile.codex_home.display());
            }
        }
        return Ok(());
    };

    let profile = state
        .profiles
        .get(active)
        .with_context(|| format!("active profile '{}' is missing", active))?;

    println!("{active}");
    println!("CODEX_HOME: {}", profile.codex_home.display());
    println!("Managed: {}", profile.managed);
    println!("Auth: {}", read_auth_summary(&profile.codex_home).label);
    Ok(())
}

fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let codex_home = default_codex_home()?;

    println!("Prodex root: {}", paths.root.display());
    println!(
        "State file: {} ({})",
        paths.state_file.display(),
        if paths.state_file.exists() {
            "exists"
        } else {
            "missing"
        }
    );
    println!(
        "Managed profiles root: {}",
        paths.managed_profiles_root.display()
    );
    println!(
        "Default CODEX_HOME: {} ({})",
        codex_home.display(),
        if codex_home.exists() {
            "exists"
        } else {
            "missing"
        }
    );
    println!("Codex binary: {}", format_binary_resolution(&codex_bin()));
    println!("CQ binary: {}", format_binary_resolution(&cq_bin()));
    println!("Profiles: {}", state.profiles.len());
    println!(
        "Active profile: {}",
        state.active_profile.as_deref().unwrap_or("-")
    );

    if state.profiles.is_empty() {
        return Ok(());
    }

    for (name, profile) in &state.profiles {
        let active = if state.active_profile.as_deref() == Some(name.as_str()) {
            " [active]"
        } else {
            ""
        };
        let auth = read_auth_summary(&profile.codex_home);
        let kind = if profile.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        println!("{name}{active}");
        println!("  kind: {kind}");
        println!("  auth: {}", auth.label);
        println!("  path: {}", profile.codex_home.display());
        println!(
            "  exists: {}",
            if profile.codex_home.exists() {
                "yes"
            } else {
                "no"
            }
        );

        if args.quota {
            match fetch_usage(&profile.codex_home, None) {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, false);
                    if blocked.is_empty() {
                        println!("  quota: ready");
                    } else {
                        println!("  quota: blocked ({})", format_blocked_limits(&blocked));
                    }
                    println!("  main: {}", format_main_windows(&usage));
                }
                Err(err) => {
                    println!("  quota: error ({})", first_line_of_error(&err.to_string()));
                }
            }
        }
    }

    Ok(())
}

fn handle_codex_login(args: CodexPassthroughArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    create_codex_home_if_missing(&codex_home)?;

    let mut command_args = vec![OsString::from("login")];
    command_args.extend(args.codex_args);

    let status = run_child(&codex_bin(), &command_args, &codex_home)?;
    if status.success() {
        state.active_profile = Some(profile_name.clone());
        state.save(&paths)?;
    }
    exit_with_status(status)
}

fn handle_codex_logout(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, selector.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    let status = run_child(&codex_bin(), &[OsString::from("logout")], &codex_home)?;
    exit_with_status(status)
}

fn handle_quota(args: QuotaArgs) -> Result<()> {
    if args.all && args.watch {
        bail!("--all cannot be combined with --watch");
    }

    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if args.all {
        if state.profiles.is_empty() {
            bail!("no profiles configured");
        }
        let mut reports = Vec::new();
        for (name, profile) in &state.profiles {
            let auth = read_auth_summary(&profile.codex_home);
            let result = fetch_usage(&profile.codex_home, args.base_url.as_deref())
                .map_err(|err| err.to_string());
            reports.push(QuotaReport {
                name: name.clone(),
                active: state.active_profile.as_deref() == Some(name.as_str()),
                auth,
                result,
            });
        }
        print_quota_reports(&reports);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    let mut command_args = if args.raw {
        vec![OsString::from("--raw")]
    } else if args.watch {
        Vec::new()
    } else {
        vec![OsString::from("--once")]
    };

    if let Some(base_url) = args.base_url {
        command_args.push(OsString::from("--base-url"));
        command_args.push(OsString::from(base_url));
    }

    let status = run_child(&cq_bin(), &command_args, &codex_home)?;
    exit_with_status(status)
}

fn handle_run(args: RunArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let include_code_review = is_review_invocation(&args.codex_args);
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if !args.skip_quota_check {
        match fetch_usage(&codex_home, args.base_url.as_deref()) {
            Ok(usage) => {
                let blocked = collect_blocked_limits(&usage, include_code_review);
                if !blocked.is_empty() {
                    eprintln!(
                        "Quota preflight blocked profile '{}': {}",
                        profile_name,
                        format_blocked_limits(&blocked)
                    );
                    let alternatives = find_ready_profiles(
                        &state,
                        &profile_name,
                        args.base_url.as_deref(),
                        include_code_review,
                    );
                    if !alternatives.is_empty() {
                        eprintln!(
                            "Other profiles that look ready: {}",
                            alternatives.join(", ")
                        );
                        eprintln!(
                            "Switch manually with `prodex use <name>` or rerun with `prodex run --profile <name> ...`."
                        );
                    }
                    eprintln!(
                        "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                        profile_name
                    );
                    std::process::exit(2);
                }
            }
            Err(err) => {
                eprintln!(
                    "Warning: quota preflight failed for '{}': {err:#}",
                    profile_name
                );
                eprintln!("Continuing without quota gate.");
            }
        }
    }

    let status = run_child(&codex_bin(), &args.codex_args, &codex_home)?;
    exit_with_status(status)
}

fn resolve_profile_name(state: &AppState, requested: Option<&str>) -> Result<String> {
    if let Some(name) = requested {
        if state.profiles.contains_key(name) {
            return Ok(name.to_string());
        }
        bail!("profile '{}' does not exist", name);
    }

    if let Some(active) = state.active_profile.as_deref() {
        if state.profiles.contains_key(active) {
            return Ok(active.to_string());
        }
        bail!("active profile '{}' no longer exists", active);
    }

    if state.profiles.len() == 1 {
        let (name, _) = state
            .profiles
            .iter()
            .next()
            .context("single profile lookup failed unexpectedly")?;
        return Ok(name.clone());
    }

    bail!("no active profile selected; use `prodex profile use <name>` or pass --profile")
}

fn ensure_path_is_unique(state: &AppState, candidate: &Path) -> Result<()> {
    for (name, profile) in &state.profiles {
        if same_path(&profile.codex_home, candidate) {
            bail!(
                "path {} is already used by profile '{}'",
                candidate.display(),
                name
            );
        }
    }
    Ok(())
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("profile name cannot be empty");
    }

    if name.contains(std::path::MAIN_SEPARATOR) {
        bail!("profile name cannot contain path separators");
    }

    if name == "." || name == ".." {
        bail!("profile name cannot be '.' or '..'");
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("profile name may only contain letters, numbers, '.', '_' or '-'");
    }

    Ok(())
}

fn copy_codex_home(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        bail!("copy source {} is not a directory", source.display());
    }

    if same_path(source, destination) {
        bail!("copy source and destination are the same path");
    }

    if destination.exists() && !dir_is_empty(destination)? {
        bail!(
            "destination {} already exists and is not empty",
            destination.display()
        );
    }

    create_codex_home_if_missing(destination)?;
    copy_directory_contents(source, destination)
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;

        if file_type.is_dir() {
            create_codex_home_if_missing(&destination_path)?;
            copy_directory_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let target = fs::read_link(&source_path)
                    .with_context(|| format!("failed to read symlink {}", source_path.display()))?;
                std::os::unix::fs::symlink(target, &destination_path).with_context(|| {
                    format!("failed to recreate symlink {}", destination_path.display())
                })?;
            }
            #[cfg(not(unix))]
            {
                bail!("symlinks are not supported on this platform");
            }
        }
    }

    Ok(())
}

fn create_codex_home_if_missing(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        let _ = fs::set_permissions(path, permissions);
    }
    Ok(())
}

fn dir_is_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries =
        fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(entries.next().is_none())
}

fn run_child(binary: &OsString, args: &[OsString], codex_home: &Path) -> Result<ExitStatus> {
    let status = Command::new(binary)
        .args(args)
        .env("CODEX_HOME", codex_home)
        .status()
        .with_context(|| format!("failed to execute {}", binary.to_string_lossy()))?;
    Ok(status)
}

fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

fn fetch_usage(codex_home: &Path, base_url: Option<&str>) -> Result<UsageResponse> {
    let mut command = Command::new(cq_bin());
    command.arg("--raw").env("CODEX_HOME", codex_home);
    if let Some(url) = base_url {
        command.arg("--base-url").arg(url);
    }

    let output = command
        .output()
        .with_context(|| format!("failed to execute {}", cq_bin().to_string_lossy()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("cq exited with status {}", output.status)
        } else {
            stderr
        };
        bail!("{message}");
    }

    let usage: UsageResponse = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("invalid JSON returned by cq for {}", codex_home.display()))?;

    Ok(usage)
}

fn print_quota_reports(reports: &[QuotaReport]) {
    let mut rows = Vec::new();
    let mut widths = [
        "NAME".len(),
        "ACT".len(),
        "AUTH".len(),
        "EMAIL".len(),
        "PLAN".len(),
        "MAIN".len(),
    ];

    for report in reports {
        let active = if report.active { "*" } else { "" }.to_string();
        let auth = report.auth.label.clone();

        let (email, plan, main, status) = match &report.result {
            Ok(usage) => {
                let blocked = collect_blocked_limits(usage, false);
                let status = if blocked.is_empty() {
                    "ready".to_string()
                } else {
                    format!("blocked: {}", format_blocked_limits(&blocked))
                };
                (
                    display_optional(usage.email.as_deref()).to_string(),
                    display_optional(usage.plan_type.as_deref()).to_string(),
                    format_main_windows_compact(usage),
                    status,
                )
            }
            Err(err) => (
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                format!("error: {}", first_line_of_error(err)),
            ),
        };

        widths[0] = widths[0].max(report.name.len());
        widths[1] = widths[1].max(active.len());
        widths[2] = widths[2].max(auth.len());
        widths[3] = widths[3].max(email.len());
        widths[4] = widths[4].max(plan.len());
        widths[5] = widths[5].max(main.len());

        rows.push((report.name.clone(), active, auth, email, plan, main, status));
    }

    println!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}  STATUS",
        "NAME",
        "ACT",
        "AUTH",
        "EMAIL",
        "PLAN",
        "MAIN",
        name_w = widths[0],
        act_w = widths[1],
        auth_w = widths[2],
        email_w = widths[3],
        plan_w = widths[4],
        main_w = widths[5],
    );

    for (name, active, auth, email, plan, main, status) in rows {
        println!(
            "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}  {}",
            name,
            active,
            auth,
            email,
            plan,
            main,
            status,
            name_w = widths[0],
            act_w = widths[1],
            auth_w = widths[2],
            email_w = widths[3],
            plan_w = widths[4],
            main_w = widths[5],
        );
    }
}

fn format_main_windows(usage: &UsageResponse) -> String {
    let Some(rate_limit) = usage.rate_limit.as_ref() else {
        return "-".to_string();
    };

    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_main_windows_compact(usage: &UsageResponse) -> String {
    let Some(rate_limit) = usage.rate_limit.as_ref() else {
        return "-".to_string();
    };

    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status_compact(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status_compact(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_window_status(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    let reset = format_reset_time(window.reset_at);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(window.used_percent);
            format!("{label}: used {used}/100, left {remaining}/100, resets {reset}")
        }
        None => format!("{label}: usage unknown, resets {reset}"),
    }
}

fn format_window_status_compact(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    match window.used_percent {
        Some(used) => format!("{label} {used}/100 used"),
        None => format!("{label} ?"),
    }
}

fn collect_blocked_limits(usage: &UsageResponse, include_code_review: bool) -> Vec<BlockedLimit> {
    let mut blocked = Vec::new();

    if let Some(main) = usage.rate_limit.as_ref() {
        push_blocked_window(&mut blocked, None, main.primary_window.as_ref());
        push_blocked_window(&mut blocked, None, main.secondary_window.as_ref());
    }

    for additional in &usage.additional_rate_limits {
        let label = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref());
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.primary_window.as_ref(),
        );
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.secondary_window.as_ref(),
        );
    }

    if include_code_review {
        if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
            push_blocked_window(
                &mut blocked,
                Some("code-review"),
                code_review.primary_window.as_ref(),
            );
            push_blocked_window(
                &mut blocked,
                Some("code-review"),
                code_review.secondary_window.as_ref(),
            );
        }
    }

    blocked
}

fn push_blocked_window(
    blocked: &mut Vec<BlockedLimit>,
    name: Option<&str>,
    window: Option<&UsageWindow>,
) {
    let Some(window) = window else {
        return;
    };
    let Some(used) = window.used_percent else {
        return;
    };
    if used < 100 {
        return;
    }

    let label = match name {
        Some(base) if !base.is_empty() => {
            format!("{base} {}", window_label(window.limit_window_seconds))
        }
        _ => window_label(window.limit_window_seconds),
    };

    blocked.push(BlockedLimit {
        label,
        reset_at: window.reset_at,
    });
}

fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| {
            format!(
                "{} until {}",
                limit.label,
                format_reset_time(limit.reset_at)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    (100 - used).clamp(0, 100)
}

fn window_label(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return "usage".to_string();
    };

    if (17_700..=18_300).contains(&seconds) {
        return "5h".to_string();
    }
    if (601_200..=608_400).contains(&seconds) {
        return "weekly".to_string();
    }
    if (2_505_600..=2_678_400).contains(&seconds) {
        return "monthly".to_string();
    }

    format!("{seconds}s")
}

fn format_reset_time(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M %Z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn display_optional(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

fn read_auth_summary(codex_home: &Path) -> AuthSummary {
    let auth_path = codex_home.join("auth.json");
    if !auth_path.is_file() {
        return AuthSummary {
            label: "no-auth".to_string(),
            quota_compatible: false,
        };
    }

    let content = match fs::read_to_string(&auth_path) {
        Ok(content) => content,
        Err(_) => {
            return AuthSummary {
                label: "unreadable-auth".to_string(),
                quota_compatible: false,
            };
        }
    };

    let stored_auth: StoredAuth = match serde_json::from_str(&content) {
        Ok(auth) => auth,
        Err(_) => {
            return AuthSummary {
                label: "invalid-auth".to_string(),
                quota_compatible: false,
            };
        }
    };

    let has_chatgpt_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.access_token.as_deref())
        .is_some_and(|token| !token.trim().is_empty());
    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());

    if has_chatgpt_token {
        return AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        };
    }

    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        return AuthSummary {
            label: "api-key".to_string(),
            quota_compatible: false,
        };
    }

    AuthSummary {
        label: stored_auth
            .auth_mode
            .unwrap_or_else(|| "auth-present".to_string()),
        quota_compatible: false,
    }
}

fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
) -> Vec<String> {
    let mut ready = Vec::new();

    for (name, profile) in &state.profiles {
        if name == current_profile {
            continue;
        }

        let auth = read_auth_summary(&profile.codex_home);
        if !auth.quota_compatible {
            continue;
        }

        if let Ok(usage) = fetch_usage(&profile.codex_home, base_url) {
            if collect_blocked_limits(&usage, include_code_review).is_empty() {
                ready.push(name.clone());
            }
        }
    }

    ready
}

fn format_binary_resolution(binary: &OsString) -> String {
    let configured = binary.to_string_lossy();
    match resolve_binary_path(binary) {
        Some(path) => format!("{configured} ({})", path.display()),
        None => format!("{configured} (not found)"),
    }
}

fn resolve_binary_path(binary: &OsString) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary);
    if candidate.components().count() > 1 {
        if candidate.is_file() {
            return Some(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let full_path = directory.join(&candidate);
        if full_path.is_file() {
            return Some(full_path);
        }
    }

    None
}

fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    Ok(current_dir.join(path))
}

fn default_codex_home() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CODEX_DIR))
}

impl AppPaths {
    fn discover() -> Result<Self> {
        let root = match env::var_os("PRODEX_HOME") {
            Some(path) => absolutize(PathBuf::from(path))?,
            None => home_dir()
                .context("failed to determine home directory")?
                .join(DEFAULT_PRODEX_DIR),
        };

        Ok(Self {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            root,
        })
    }
}

impl AppState {
    fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.state_file.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&paths.state_file)
            .with_context(|| format!("failed to read {}", paths.state_file.display()))?;
        let state = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", paths.state_file.display()))?;
        Ok(state)
    }

    fn save(&self, paths: &AppPaths) -> Result<()> {
        fs::create_dir_all(&paths.root)
            .with_context(|| format!("failed to create {}", paths.root.display()))?;

        let json =
            serde_json::to_string_pretty(self).context("failed to serialize prodex state")?;
        let temp_file = paths.state_file.with_extension("json.tmp");
        fs::write(&temp_file, json)
            .with_context(|| format!("failed to write {}", temp_file.display()))?;
        fs::rename(&temp_file, &paths.state_file).with_context(|| {
            format!(
                "failed to replace state file {}",
                paths.state_file.display()
            )
        })?;
        Ok(())
    }
}

fn codex_bin() -> OsString {
    env::var_os("PRODEX_CODEX_BIN").unwrap_or_else(|| OsString::from("codex"))
}

fn cq_bin() -> OsString {
    env::var_os("PRODEX_CQ_BIN").unwrap_or_else(|| OsString::from("cq"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_profile_names() {
        assert!(validate_profile_name("alpha-1").is_ok());
        assert!(validate_profile_name("bad/name").is_err());
        assert!(validate_profile_name("bad space").is_err());
    }

    #[test]
    fn recognizes_known_windows() {
        assert_eq!(window_label(Some(18_000)), "5h");
        assert_eq!(window_label(Some(604_800)), "weekly");
        assert_eq!(window_label(Some(2_592_000)), "monthly");
    }

    #[test]
    fn blocks_when_main_window_is_exhausted() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(100),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let blocked = collect_blocked_limits(&usage, false);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].label, "5h");
    }

    #[test]
    fn compact_window_format_uses_scale_of_100() {
        let window = UsageWindow {
            used_percent: Some(37),
            reset_at: None,
            limit_window_seconds: Some(18_000),
        };

        assert_eq!(format_window_status_compact(&window), "5h 37/100 used");
        assert!(format_window_status(&window).contains("used 37/100"));
        assert!(format_window_status(&window).contains("left 63/100"));
    }
}
