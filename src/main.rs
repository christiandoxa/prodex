use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::{Local, TimeZone};
use clap::{Args, Parser, Subcommand};
use dirs::home_dir;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_PRODEX_DIR: &str = ".prodex";
const DEFAULT_CODEX_DIR: &str = ".codex";
const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api";
const DEFAULT_WATCH_INTERVAL_SECONDS: u64 = 5;
const CLI_WIDTH: usize = 110;
const CLI_LABEL_WIDTH: usize = 16;
const CLI_TABLE_GAP: &str = "  ";
const SHARED_CODEX_DIR_NAMES: &[&str] = &["sessions", "archived_sessions", "shell_snapshots"];
const SHARED_CODEX_FILE_NAMES: &[&str] = &["history.jsonl"];
const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];

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
    detail: bool,
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
    #[arg(long, conflicts_with = "no_auto_rotate")]
    auto_rotate: bool,
    #[arg(long)]
    no_auto_rotate: bool,
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
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Clone)]
struct AppPaths {
    root: PathBuf,
    state_file: PathBuf,
    managed_profiles_root: PathBuf,
    shared_codex_root: PathBuf,
    legacy_shared_codex_root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageResponse {
    email: Option<String>,
    plan_type: Option<String>,
    rate_limit: Option<WindowPair>,
    code_review_rate_limit: Option<WindowPair>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    account_id: Option<String>,
    id_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    profile: Option<IdTokenProfileClaims>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenProfileClaims {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug)]
struct BlockedLimit {
    message: String,
}

#[derive(Debug, Clone)]
struct AuthSummary {
    label: String,
    quota_compatible: bool,
}

#[derive(Debug, Clone)]
struct UsageAuth {
    access_token: String,
    account_id: Option<String>,
}

#[derive(Debug)]
struct QuotaReport {
    name: String,
    active: bool,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug)]
struct QuotaFetchJob {
    name: String,
    active: bool,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct ProfileSummaryJob {
    name: String,
    active: bool,
    managed: bool,
    email: Option<String>,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct ProfileSummaryReport {
    name: String,
    active: bool,
    managed: bool,
    auth: AuthSummary,
    email: Option<String>,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct DoctorProfileReport {
    summary: ProfileSummaryReport,
    quota: Option<std::result::Result<UsageResponse, String>>,
}

#[derive(Debug)]
struct ProfileEmailLookupJob {
    name: String,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct RunProfileProbeJob {
    name: String,
    order_index: usize,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct RunProfileProbeReport {
    name: String,
    order_index: usize,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone)]
struct ReadyProfileCandidate {
    name: String,
    usage: UsageResponse,
    order_index: usize,
    preferred: bool,
}

#[derive(Debug, Clone, Copy)]
struct MainWindowSnapshot {
    remaining_percent: i64,
    reset_at: i64,
    pressure_score: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedCodexEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedCodexEntry {
    name: String,
    kind: SharedCodexEntryKind,
}

fn section_header(title: &str) -> String {
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= CLI_WIDTH {
        return prefix;
    }

    format!("{prefix}{}", "=".repeat(CLI_WIDTH - width))
}

fn text_width(value: &str) -> usize {
    value.chars().count()
}

fn fit_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    if text_width(value) <= width {
        return value.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut output = String::new();
    for ch in value.chars().take(width - 3) {
        output.push(ch);
    }
    output.push_str("...");
    output
}

fn chunk_token(token: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if token.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in token.chars() {
        current.push(ch);
        if text_width(&current) >= width {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            for piece in chunk_token(word, width) {
                if current.is_empty() {
                    current.push_str(&piece);
                } else if text_width(&current) + 1 + text_width(&piece) <= width {
                    current.push(' ');
                    current.push_str(&piece);
                } else {
                    lines.push(std::mem::take(&mut current));
                    current.push_str(&piece);
                }
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn format_field_lines(label: &str, value: &str) -> Vec<String> {
    let label = format!("{label}:");
    let value_width = CLI_WIDTH.saturating_sub(CLI_LABEL_WIDTH + 1).max(1);
    let wrapped = wrap_text(value, value_width);
    let mut lines = Vec::new();

    for (index, line) in wrapped.into_iter().enumerate() {
        let field_label = if index == 0 { label.as_str() } else { "" };
        lines.push(format!(
            "{field_label:<label_w$} {line}",
            label_w = CLI_LABEL_WIDTH
        ));
    }

    lines
}

fn print_panel(title: &str, fields: &[(String, String)]) {
    println!("{}", section_header(title));
    for (label, value) in fields {
        for line in format_field_lines(label, value) {
            println!("{line}");
        }
    }
}

fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    let mut lines = vec![section_header(title)];
    for (label, value) in fields {
        lines.extend(format_field_lines(label, value));
    }
    lines.join("\n")
}

fn print_wrapped_stderr(message: &str) {
    for line in wrap_text(message, CLI_WIDTH) {
        eprintln!("{line}");
    }
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

fn handle_current_profile() -> Result<()> {
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

fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let codex_home = default_codex_home()?;

    let summary_fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "State file".to_string(),
            format!(
                "{} ({})",
                paths.state_file.display(),
                if paths.state_file.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Profiles root".to_string(),
            paths.managed_profiles_root.display().to_string(),
        ),
        (
            "Default CODEX_HOME".to_string(),
            format!(
                "{} ({})",
                codex_home.display(),
                if codex_home.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Codex binary".to_string(),
            format_binary_resolution(&codex_bin()),
        ),
        (
            "Quota endpoint".to_string(),
            usage_url(&quota_base_url(None)),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Doctor", &summary_fields);

    if state.profiles.is_empty() {
        return Ok(());
    }

    for report in collect_doctor_profile_reports(&state, args.quota) {
        let kind = if report.summary.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        let mut fields = vec![
            (
                "Current".to_string(),
                if report.summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            ("Auth".to_string(), report.summary.auth.label),
            (
                "Email".to_string(),
                report.summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "Path".to_string(),
                report.summary.codex_home.display().to_string(),
            ),
            (
                "Exists".to_string(),
                if report.summary.codex_home.exists() {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
        ];

        if let Some(quota) = report.quota {
            match quota {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, false);
                    fields.push((
                        "Quota".to_string(),
                        if blocked.is_empty() {
                            "Ready".to_string()
                        } else {
                            format!("Blocked ({})", format_blocked_limits(&blocked))
                        },
                    ));
                    fields.push(("Main".to_string(), format_main_windows(&usage)));
                }
                Err(err) => {
                    fields.push((
                        "Quota".to_string(),
                        format!("Error ({})", first_line_of_error(&err.to_string())),
                    ));
                }
            }
        }
        print_panel(&format!("Profile {}", report.summary.name), &fields);
    }

    Ok(())
}

fn handle_codex_login(args: CodexPassthroughArgs) -> Result<()> {
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
    run_child(&codex_bin(), &command_args, codex_home)
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

fn fetch_profile_email(codex_home: &Path) -> Result<String> {
    let auth_email_error = match read_profile_email_from_auth(codex_home) {
        Ok(Some(email)) => return Ok(email),
        Ok(None) => None,
        Err(err) => Some(err),
    };

    match fetch_profile_email_from_usage(codex_home) {
        Ok(email) => Ok(email),
        Err(usage_error) => {
            if let Some(auth_error) = auth_email_error {
                bail!(
                    "failed to read account email from auth.json ({auth_error:#}) and quota endpoint ({usage_error:#})"
                );
            }
            Err(usage_error)
        }
    }
}

fn read_profile_email_from_auth(codex_home: &Path) -> Result<Option<String>> {
    let auth_path = codex_home.join("auth.json");
    if !auth_path.is_file() {
        return Ok(None);
    }

    let content = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_path.display()))?;
    let id_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.id_token.as_deref())
        .map(str::trim)
        .filter(|token| !token.is_empty());

    let Some(id_token) = id_token else {
        return Ok(None);
    };

    parse_email_from_id_token(id_token)
        .with_context(|| format!("failed to parse id_token in {}", auth_path.display()))
}

fn parse_email_from_id_token(raw_jwt: &str) -> Result<Option<String>> {
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
    let claims: IdTokenClaims =
        serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")?;

    Ok(claims
        .email
        .or_else(|| claims.profile.and_then(|profile| profile.email))
        .map(|email| email.trim().to_string())
        .filter(|email| !email.is_empty()))
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

fn find_profile_by_email(state: &mut AppState, email: &str) -> Result<Option<String>> {
    let target_email = normalize_email(email);
    let summaries = collect_profile_summaries(state);

    for summary in &summaries {
        if summary
            .email
            .as_deref()
            .is_some_and(|cached| normalize_email(cached) == target_email)
        {
            return Ok(Some(summary.name.clone()));
        }
    }

    let discovered = map_parallel(
        summaries
            .into_iter()
            .filter_map(|summary| {
                if summary.email.is_some() || !summary.auth.quota_compatible {
                    return None;
                }

                Some(ProfileEmailLookupJob {
                    name: summary.name,
                    codex_home: summary.codex_home,
                })
            })
            .collect(),
        |job| (job.name, fetch_profile_email(&job.codex_home).ok()),
    );

    let mut matched_profile = None;
    for (name, fetched_email) in discovered {
        let Some(fetched_email) = fetched_email else {
            continue;
        };

        if matched_profile.is_none() && normalize_email(&fetched_email) == target_email {
            matched_profile = Some(name.clone());
        }
        if let Some(profile) = state.profiles.get_mut(&name) {
            profile.email = Some(fetched_email);
        }
    }

    Ok(matched_profile)
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn profile_name_from_email(email: &str) -> String {
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

fn unique_profile_name_for_email(paths: &AppPaths, state: &AppState, email: &str) -> String {
    let base_name = profile_name_from_email(email);
    if is_available_profile_name(paths, state, &base_name) {
        return base_name;
    }

    for suffix in 2.. {
        let candidate = format!("{base_name}-{suffix}");
        if is_available_profile_name(paths, state, &candidate) {
            return candidate;
        }
    }

    unreachable!("integer suffix space should not be exhausted")
}

fn is_available_profile_name(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn persist_login_home(source: &Path, destination: &Path) -> Result<()> {
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

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).with_context(|| format!("failed to delete {}", path.display()))
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
        let reports = collect_quota_reports(&state, args.base_url.as_deref());
        print_quota_reports(&reports, args.detail);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if args.raw {
        let usage = fetch_usage_json(&codex_home, args.base_url.as_deref())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?
        );
        return Ok(());
    }

    if args.watch {
        return watch_quota(&profile_name, &codex_home, args.base_url.as_deref());
    }

    let usage = fetch_usage(&codex_home, args.base_url.as_deref())?;
    println!("{}", render_profile_quota(&profile_name, &usage));
    Ok(())
}

fn handle_run(args: RunArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let mut selected_profile_name = profile_name.clone();
    let explicit_profile_requested = args.profile.is_some();
    let allow_auto_rotate = !args.no_auto_rotate;
    let include_code_review = is_review_invocation(&args.codex_args);
    let mut codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if !args.skip_quota_check {
        if allow_auto_rotate && !explicit_profile_requested && state.profiles.len() > 1 {
            let reports = collect_run_profile_reports(
                &state,
                active_profile_selection_order(&state, &profile_name),
                args.base_url.as_deref(),
            );
            let ready_candidates =
                ready_profile_candidates(&reports, include_code_review, Some(&profile_name));
            let selected_report = reports.iter().find(|report| report.name == profile_name);

            if let Some(best_candidate) = ready_candidates.first() {
                if best_candidate.name != profile_name {
                    print_wrapped_stderr(&section_header("Quota Preflight"));
                    let mut selection_message = format!(
                        "Using profile '{}' ({})",
                        best_candidate.name,
                        format_main_windows_compact(&best_candidate.usage)
                    );
                    if let Some(report) = selected_report {
                        match &report.result {
                            Ok(usage) => {
                                let blocked = collect_blocked_limits(usage, include_code_review);
                                if !blocked.is_empty() {
                                    print_wrapped_stderr(&format!(
                                        "Quota preflight blocked profile '{}': {}",
                                        profile_name,
                                        format_blocked_limits(&blocked)
                                    ));
                                    selection_message = format!(
                                        "Auto-rotating to profile '{}' using quota-pressure scoring ({}).",
                                        best_candidate.name,
                                        format_main_windows_compact(&best_candidate.usage)
                                    );
                                } else {
                                    selection_message = format!(
                                        "Auto-selecting profile '{}' over active profile '{}' using quota-pressure scoring ({}).",
                                        best_candidate.name,
                                        profile_name,
                                        format_main_windows_compact(&best_candidate.usage)
                                    );
                                }
                            }
                            Err(err) => {
                                print_wrapped_stderr(&format!(
                                    "Warning: quota preflight failed for '{}': {err}",
                                    profile_name
                                ));
                                selection_message = format!(
                                    "Using ready profile '{}' after quota preflight failed ({})",
                                    best_candidate.name,
                                    format_main_windows_compact(&best_candidate.usage)
                                );
                            }
                        }
                    }

                    codex_home = state
                        .profiles
                        .get(&best_candidate.name)
                        .with_context(|| format!("profile '{}' is missing", best_candidate.name))?
                        .codex_home
                        .clone();
                    selected_profile_name = best_candidate.name.clone();
                    state.active_profile = Some(best_candidate.name.clone());
                    state.save(&paths)?;
                    print_wrapped_stderr(&selection_message);
                }
            } else if let Some(report) = selected_report {
                match &report.result {
                    Ok(usage) => {
                        let blocked = collect_blocked_limits(usage, include_code_review);
                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Quota preflight blocked profile '{}': {}",
                            profile_name,
                            format_blocked_limits(&blocked)
                        ));
                        print_wrapped_stderr("No ready profile was found.");
                        print_wrapped_stderr(&format!(
                            "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                            profile_name
                        ));
                        std::process::exit(2);
                    }
                    Err(err) => {
                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Warning: quota preflight failed for '{}': {err:#}",
                            profile_name
                        ));
                        print_wrapped_stderr("Continuing without quota gate.");
                    }
                }
            }
        } else {
            match fetch_usage(&codex_home, args.base_url.as_deref()) {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, include_code_review);
                    if !blocked.is_empty() {
                        let alternatives = find_ready_profiles(
                            &state,
                            &profile_name,
                            args.base_url.as_deref(),
                            include_code_review,
                        );

                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Quota preflight blocked profile '{}': {}",
                            profile_name,
                            format_blocked_limits(&blocked)
                        ));

                        if allow_auto_rotate {
                            if let Some(next_profile) = alternatives.first() {
                                let next_profile = next_profile.clone();
                                codex_home = state
                                    .profiles
                                    .get(&next_profile)
                                    .with_context(|| {
                                        format!("profile '{}' is missing", next_profile)
                                    })?
                                    .codex_home
                                    .clone();
                                selected_profile_name = next_profile.clone();
                                state.active_profile = Some(next_profile.clone());
                                state.save(&paths)?;
                                print_wrapped_stderr(&format!(
                                    "Auto-rotating to profile '{}'.",
                                    next_profile
                                ));
                            } else {
                                print_wrapped_stderr("No other ready profile was found.");
                                print_wrapped_stderr(&format!(
                                    "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                    profile_name
                                ));
                                std::process::exit(2);
                            }
                        } else {
                            if !alternatives.is_empty() {
                                print_wrapped_stderr(&format!(
                                    "Other profiles that look ready: {}",
                                    alternatives.join(", ")
                                ));
                                print_wrapped_stderr(
                                    "Rerun without `--no-auto-rotate` to allow fallback.",
                                );
                            }
                            print_wrapped_stderr(&format!(
                                "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                profile_name
                            ));
                            std::process::exit(2);
                        }
                    }
                }
                Err(err) => {
                    print_wrapped_stderr(&section_header("Quota Preflight"));
                    print_wrapped_stderr(&format!(
                        "Warning: quota preflight failed for '{}': {err:#}",
                        profile_name
                    ));
                    print_wrapped_stderr("Continuing without quota gate.");
                }
            }
        }
    }

    if state
        .profiles
        .get(&selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?
        .managed
    {
        prepare_managed_codex_home(&paths, &codex_home)?;
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

fn prepare_managed_codex_home(paths: &AppPaths, codex_home: &Path) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    migrate_legacy_shared_codex_root(paths)?;
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    for entry in shared_codex_entries(paths, codex_home)? {
        ensure_shared_codex_entry(paths, codex_home, &entry)?;
    }

    Ok(())
}

fn migrate_legacy_shared_codex_root(paths: &AppPaths) -> Result<()> {
    if same_path(&paths.shared_codex_root, &paths.legacy_shared_codex_root)
        || !paths.legacy_shared_codex_root.exists()
    {
        return Ok(());
    }

    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    collect_shared_codex_sqlite_entries(&paths.legacy_shared_codex_root, &mut sqlite_entries)?;
    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    for entry in entries {
        let legacy_path = paths.legacy_shared_codex_root.join(&entry.name);
        let shared_path = paths.shared_codex_root.join(&entry.name);
        migrate_shared_codex_entry(&legacy_path, &shared_path, entry.kind)?;
    }

    Ok(())
}

fn shared_codex_entries(paths: &AppPaths, codex_home: &Path) -> Result<Vec<SharedCodexEntry>> {
    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    let mut scan_roots = vec![paths.shared_codex_root.clone(), codex_home.to_path_buf()];
    scan_roots.sort();
    scan_roots.dedup();

    for root in scan_roots {
        collect_shared_codex_sqlite_entries(&root, &mut sqlite_entries)?;
    }

    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    Ok(entries)
}

fn collect_shared_codex_sqlite_entries(root: &Path, names: &mut BTreeSet<String>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_shared_codex_sqlite_name(&file_name) {
            names.insert(file_name.into_owned());
        }
    }

    Ok(())
}

fn is_shared_codex_sqlite_name(file_name: &str) -> bool {
    SHARED_CODEX_SQLITE_PREFIXES
        .iter()
        .any(|prefix| file_name.starts_with(prefix))
        && SHARED_CODEX_SQLITE_SUFFIXES
            .iter()
            .any(|suffix| file_name.ends_with(suffix))
}

fn ensure_shared_codex_entry(
    paths: &AppPaths,
    codex_home: &Path,
    entry: &SharedCodexEntry,
) -> Result<()> {
    let local_path = codex_home.join(&entry.name);
    let shared_path = paths.shared_codex_root.join(&entry.name);
    if let Some(parent) = shared_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    migrate_shared_codex_entry(&local_path, &shared_path, entry.kind)?;

    if entry.kind == SharedCodexEntryKind::Directory && !shared_path.exists() {
        create_codex_home_if_missing(&shared_path)?;
    }

    ensure_symlink_to_shared(&local_path, &shared_path, entry.kind)
}

fn migrate_shared_codex_entry(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(local_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", local_path.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        remove_path(local_path)?;
        return Ok(());
    }

    match kind {
        SharedCodexEntryKind::Directory => {
            if !metadata.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex session state",
                    local_path.display()
                );
            }

            if !shared_path.exists() {
                move_directory(local_path, shared_path)?;
                return Ok(());
            }
            if !shared_path.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex session state",
                    shared_path.display()
                );
            }

            copy_directory_contents(local_path, shared_path)?;
            fs::remove_dir_all(local_path)
                .with_context(|| format!("failed to remove {}", local_path.display()))?;
        }
        SharedCodexEntryKind::File => {
            if !metadata.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex session state",
                    local_path.display()
                );
            }

            if !shared_path.exists() {
                move_file(local_path, shared_path)?;
                return Ok(());
            }
            if !shared_path.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex session state",
                    shared_path.display()
                );
            }

            if local_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "history.jsonl")
            {
                append_file_contents(local_path, shared_path)?;
            }

            fs::remove_file(local_path)
                .with_context(|| format!("failed to remove {}", local_path.display()))?;
        }
    }

    Ok(())
}

fn move_directory(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            create_codex_home_if_missing(destination)?;
            copy_directory_contents(source, destination)?;
            fs::remove_dir_all(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(source, destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

fn append_file_contents(source: &Path, destination: &Path) -> Result<()> {
    let content =
        fs::read(source).with_context(|| format!("failed to read {}", source.display()))?;
    if content.is_empty() {
        return Ok(());
    }

    use std::io::Write as _;

    let mut destination_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destination)
        .with_context(|| format!("failed to open {}", destination.display()))?;

    let destination_len = destination_file
        .metadata()
        .with_context(|| format!("failed to inspect {}", destination.display()))?
        .len();
    if destination_len > 0 {
        destination_file
            .write_all(b"\n")
            .with_context(|| format!("failed to append separator to {}", destination.display()))?;
    }

    destination_file.write_all(&content).with_context(|| {
        format!(
            "failed to append {} to {}",
            source.display(),
            destination.display()
        )
    })
}

fn ensure_symlink_to_shared(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    if local_path.exists() {
        remove_path(local_path)?;
    } else if fs::symlink_metadata(local_path).is_ok() {
        remove_path(local_path)?;
    }

    create_symlink(shared_path, local_path, kind)
}

fn create_symlink(target: &Path, link: &Path, _kind: SharedCodexEntryKind) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link shared Codex session state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        match _kind {
            SharedCodexEntryKind::Directory => std::os::windows::fs::symlink_dir(target, link),
            SharedCodexEntryKind::File => std::os::windows::fs::symlink_file(target, link),
        }
        .with_context(|| {
            format!(
                "failed to link shared Codex session state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = _kind;
        bail!("shared Codex session links are not supported on this platform");
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove symbolic link {}", path.display()))?;
        return Ok(());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
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

fn collect_quota_reports(state: &AppState, base_url: Option<&str>) -> Vec<QuotaReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| QuotaFetchJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            codex_home: profile.codex_home.clone(),
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| QuotaReport {
        name: job.name,
        active: job.active,
        auth: read_auth_summary(&job.codex_home),
        result: fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string()),
    })
}

fn collect_profile_summaries(state: &AppState) -> Vec<ProfileSummaryReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| ProfileSummaryJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            managed: profile.managed,
            email: profile.email.clone(),
            codex_home: profile.codex_home.clone(),
        })
        .collect();

    map_parallel(jobs, |job| ProfileSummaryReport {
        name: job.name,
        active: job.active,
        managed: job.managed,
        auth: read_auth_summary(&job.codex_home),
        email: job.email,
        codex_home: job.codex_home,
    })
}

fn collect_doctor_profile_reports(
    state: &AppState,
    include_quota: bool,
) -> Vec<DoctorProfileReport> {
    map_parallel(collect_profile_summaries(state), |summary| {
        DoctorProfileReport {
            quota: include_quota
                .then(|| fetch_usage(&summary.codex_home, None).map_err(|err| err.to_string())),
            summary,
        }
    })
}

fn collect_run_profile_reports(
    state: &AppState,
    profile_names: Vec<String>,
    base_url: Option<&str>,
) -> Vec<RunProfileProbeReport> {
    let jobs = profile_names
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            Some(RunProfileProbeJob {
                name,
                order_index,
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = read_auth_summary(&job.codex_home);
        let result = if auth.quota_compatible {
            fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };

        RunProfileProbeReport {
            name: job.name,
            order_index: job.order_index,
            auth,
            result,
        }
    })
}

fn ready_profile_candidates(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    let mut candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let usage = report.result.as_ref().ok()?;
            if !collect_blocked_limits(usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage: usage.clone(),
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(ready_profile_sort_key);
    candidates
}

fn ready_profile_sort_key(
    candidate: &ReadyProfileCandidate,
) -> (
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
    usize,
    usize,
) {
    let weekly = required_main_window_snapshot(&candidate.usage, "weekly");
    let five_hour = required_main_window_snapshot(&candidate.usage, "5h");

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let total_pressure = weekly_pressure
        .saturating_mul(8)
        .saturating_add(five_hour_pressure);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let reserve_floor = weekly_remaining.min(five_hour_remaining);
    let weekly_reset_at = weekly.map_or(i64::MAX, |window| window.reset_at);
    let five_hour_reset_at = five_hour.map_or(i64::MAX, |window| window.reset_at);

    (
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        Reverse(reserve_floor),
        Reverse(weekly_remaining),
        Reverse(five_hour_remaining),
        weekly_reset_at,
        five_hour_reset_at,
        if candidate.preferred { 0usize } else { 1usize },
        candidate.order_index,
    )
}

fn required_main_window_snapshot(usage: &UsageResponse, label: &str) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - Local::now().timestamp()).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

fn active_profile_selection_order(state: &AppState, current_profile: &str) -> Vec<String> {
    std::iter::once(current_profile.to_string())
        .chain(profile_rotation_order(state, current_profile))
        .collect()
}

fn map_parallel<I, O, F>(inputs: Vec<I>, func: F) -> Vec<O>
where
    I: Send,
    O: Send,
    F: Fn(I) -> O + Sync,
{
    if inputs.len() <= 1 {
        return inputs.into_iter().map(func).collect();
    }

    thread::scope(|scope| {
        let func = &func;
        let mut handles = Vec::with_capacity(inputs.len());
        for input in inputs {
            handles.push(scope.spawn(move || func(input)));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel worker panicked"))
            .collect()
    })
}

fn fetch_usage(codex_home: &Path, base_url: Option<&str>) -> Result<UsageResponse> {
    let usage: UsageResponse = serde_json::from_value(fetch_usage_json(codex_home, base_url)?)
        .with_context(|| {
            format!(
                "invalid JSON returned by quota backend for {}",
                codex_home.display()
            )
        })?;
    Ok(usage)
}

fn fetch_usage_json(codex_home: &Path, base_url: Option<&str>) -> Result<serde_json::Value> {
    let auth = read_usage_auth(codex_home)?;
    let usage_url = usage_url(&quota_base_url(base_url));
    let client = Client::builder()
        .build()
        .context("failed to build quota HTTP client")?;

    let mut request = client
        .get(&usage_url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("User-Agent", "codex-cli");

    if let Some(account_id) = auth.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .with_context(|| format!("failed to request quota endpoint {}", usage_url))?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read quota response body")?;

    if !status.is_success() {
        let body_text = format_response_body(&body);
        if body_text.is_empty() {
            bail!("request failed (HTTP {}) to {}", status.as_u16(), usage_url);
        }
        bail!(
            "request failed (HTTP {}) to {}: {}",
            status.as_u16(),
            usage_url,
            body_text
        );
    }

    let usage = serde_json::from_slice(&body).with_context(|| {
        format!(
            "invalid JSON returned by quota backend for {}",
            codex_home.display()
        )
    })?;

    Ok(usage)
}

fn print_quota_reports(reports: &[QuotaReport], detail: bool) {
    const PROFILE_COL_WIDTH: usize = 24;
    const CUR_COL_WIDTH: usize = 3;
    const AUTH_COL_WIDTH: usize = 7;
    const ACCOUNT_COL_WIDTH: usize = 27;
    const PLAN_COL_WIDTH: usize = 8;
    const REMAINING_COL_WIDTH: usize = 31;

    let mut rows = Vec::new();

    for report in sort_quota_reports_for_display(reports) {
        let active = if report.active { "*" } else { "" }.to_string();
        let auth = report.auth.label.clone();

        let (email, plan, main, status, resets) = match &report.result {
            Ok(usage) => {
                let blocked = collect_blocked_limits(usage, false);
                let status = if blocked.is_empty() {
                    "Ready".to_string()
                } else {
                    format!("Blocked: {}", format_blocked_limits(&blocked))
                };
                (
                    display_optional(usage.email.as_deref()).to_string(),
                    display_optional(usage.plan_type.as_deref()).to_string(),
                    format_main_windows_compact(usage),
                    status,
                    Some(format!("resets: {}", format_main_reset_summary(usage))),
                )
            }
            Err(err) => (
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                format!("Error: {}", first_line_of_error(err)),
                Some("resets: unavailable".to_string()),
            ),
        };

        rows.push((
            report.name.clone(),
            active,
            auth,
            email,
            plan,
            main,
            status,
            resets,
        ));
    }

    println!("{}", section_header("Quota Overview"));
    let header = format!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}",
        "PROFILE",
        "CUR",
        "AUTH",
        "ACCOUNT",
        "PLAN",
        "REMAINING",
        name_w = PROFILE_COL_WIDTH,
        act_w = CUR_COL_WIDTH,
        auth_w = AUTH_COL_WIDTH,
        email_w = ACCOUNT_COL_WIDTH,
        plan_w = PLAN_COL_WIDTH,
        main_w = REMAINING_COL_WIDTH,
    );
    println!("{header}");
    println!("{}", "-".repeat(text_width(&header)));

    for (name, active, auth, email, plan, main, status, resets) in rows {
        println!(
            "{:<name_w$}{}{:<act_w$}{}{:<auth_w$}{}{:<email_w$}{}{:<plan_w$}{}{:<main_w$}",
            fit_cell(&name, PROFILE_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&active, CUR_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&auth, AUTH_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&email, ACCOUNT_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&plan, PLAN_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&main, REMAINING_COL_WIDTH),
            name_w = PROFILE_COL_WIDTH,
            act_w = CUR_COL_WIDTH,
            auth_w = AUTH_COL_WIDTH,
            email_w = ACCOUNT_COL_WIDTH,
            plan_w = PLAN_COL_WIDTH,
            main_w = REMAINING_COL_WIDTH,
        );
        for line in wrap_text(&format!("status: {status}"), CLI_WIDTH - 2) {
            println!("  {line}");
        }
        if detail {
            if let Some(resets) = resets.as_deref() {
                for line in wrap_text(resets, CLI_WIDTH - 2) {
                    println!("  {line}");
                }
            }
        }
        println!();
    }
}

fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    let mut sorted = reports.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        quota_report_sort_key(left)
            .cmp(&quota_report_sort_key(right))
            .then_with(|| left.name.cmp(&right.name))
    });
    sorted
}

fn quota_report_sort_key(report: &QuotaReport) -> (usize, i64) {
    (
        quota_report_status_rank(report),
        quota_report_earliest_main_reset_epoch(report).unwrap_or(i64::MAX),
    )
}

fn quota_report_status_rank(report: &QuotaReport) -> usize {
    match &report.result {
        Ok(usage) if collect_blocked_limits(usage, false).is_empty() => 0,
        Ok(_) => 1,
        Err(_) => 2,
    }
}

fn quota_report_earliest_main_reset_epoch(report: &QuotaReport) -> Option<i64> {
    earliest_required_main_reset_epoch(report.result.as_ref().ok()?)
}

fn earliest_required_main_reset_epoch(usage: &UsageResponse) -> Option<i64> {
    ["5h", "weekly"]
        .into_iter()
        .filter_map(|label| {
            find_main_window(usage.rate_limit.as_ref()?, label).and_then(|window| window.reset_at)
        })
        .min()
}

fn format_main_windows(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair)
        .unwrap_or_else(|| "-".to_string())
}

fn format_main_windows_compact(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair_compact)
        .unwrap_or_else(|| "-".to_string())
}

fn format_main_reset_summary(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_main_reset_pair)
        .unwrap_or_else(|| "5h unavailable | weekly unavailable".to_string())
}

fn format_main_reset_pair(rate_limit: &WindowPair) -> String {
    [
        format_main_reset_window(rate_limit, "5h"),
        format_main_reset_window(rate_limit, "weekly"),
    ]
    .join(" | ")
}

fn format_main_reset_window(rate_limit: &WindowPair, label: &str) -> String {
    match find_main_window(rate_limit, label) {
        Some(window) => {
            let reset = window
                .reset_at
                .map(|epoch| format_precise_reset_time(Some(epoch)))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{label} {reset}")
        }
        None => format!("{label} unavailable"),
    }
}

fn format_window_pair(rate_limit: &WindowPair) -> String {
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

fn format_window_pair_compact(rate_limit: &WindowPair) -> String {
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

fn format_named_window_status(label: &str, window: &UsageWindow) -> String {
    format!("{label}: {}", format_window_details(window))
}

fn format_window_status(window: &UsageWindow) -> String {
    format_named_window_status(&window_label(window.limit_window_seconds), window)
}

fn format_window_status_compact(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(Some(used));
            format!("{label} {remaining}% left")
        }
        None => format!("{label} ?"),
    }
}

fn format_window_details(window: &UsageWindow) -> String {
    let reset = format_reset_time(window.reset_at);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(window.used_percent);
            format!("{remaining}% left ({used}% used), resets {reset}")
        }
        None => format!("usage unknown, resets {reset}"),
    }
}

fn collect_blocked_limits(usage: &UsageResponse, include_code_review: bool) -> Vec<BlockedLimit> {
    let mut blocked = Vec::new();

    if let Some(main) = usage.rate_limit.as_ref() {
        push_required_main_window(&mut blocked, main, "5h");
        push_required_main_window(&mut blocked, main, "weekly");
    } else {
        blocked.push(BlockedLimit {
            message: "5h quota unavailable".to_string(),
        });
        blocked.push(BlockedLimit {
            message: "weekly quota unavailable".to_string(),
        });
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

fn push_required_main_window(
    blocked: &mut Vec<BlockedLimit>,
    main: &WindowPair,
    required_label: &str,
) {
    let Some(window) = find_main_window(main, required_label) else {
        blocked.push(BlockedLimit {
            message: format!("{required_label} quota unavailable"),
        });
        return;
    };

    match window.used_percent {
        Some(used) if used < 100 => {}
        Some(_) => blocked.push(BlockedLimit {
            message: format!(
                "{required_label} exhausted until {}",
                format_reset_time(window.reset_at)
            ),
        }),
        None => blocked.push(BlockedLimit {
            message: format!("{required_label} quota unknown"),
        }),
    }
}

fn find_main_window<'a>(main: &'a WindowPair, expected_label: &str) -> Option<&'a UsageWindow> {
    [main.primary_window.as_ref(), main.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .find(|window| window_label(window.limit_window_seconds) == expected_label)
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
        message: format!(
            "{label} exhausted until {}",
            format_reset_time(window.reset_at)
        ),
    });
}

fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.clone())
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

fn format_precise_reset_time(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %Z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn display_optional(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn render_profile_quota(profile_name: &str, usage: &UsageResponse) -> String {
    let blocked = collect_blocked_limits(usage, false);
    let status = if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!("Blocked ({})", format_blocked_limits(&blocked))
    };
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(usage.email.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(usage.plan_type.as_deref()).to_string(),
        ),
        ("Status".to_string(), status),
        ("Main".to_string(), format_main_windows(usage)),
    ];

    if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        fields.push(("Code review".to_string(), format_window_pair(code_review)));
    }

    for (name, value) in format_additional_limits(usage) {
        fields.push((name, value));
    }

    render_panel(&format!("Quota {profile_name}"), &fields)
}

fn format_additional_limits(usage: &UsageResponse) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    for additional in &usage.additional_rate_limits {
        let name = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref())
            .unwrap_or("Additional");

        if let Some(primary) = additional.rate_limit.primary_window.as_ref() {
            lines.push((
                additional_window_label(name, primary),
                format_window_details(primary),
            ));
        }
        if let Some(secondary) = additional.rate_limit.secondary_window.as_ref() {
            lines.push((
                additional_window_label(name, secondary),
                format_window_details(secondary),
            ));
        }
    }

    lines
}

fn additional_window_label(base: &str, window: &UsageWindow) -> String {
    format!("{base} {}", window_label(window.limit_window_seconds))
}

fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

fn watch_quota(profile_name: &str, codex_home: &Path, base_url: Option<&str>) -> Result<()> {
    loop {
        print!("\x1b[H\x1b[2J");
        let header = vec![
            ("Profile".to_string(), profile_name.to_string()),
            (
                "Updated".to_string(),
                Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            ),
        ];
        println!("{}", render_panel("Quota Watch", &header));
        println!();

        match fetch_usage(codex_home, base_url) {
            Ok(usage) => println!("{}", render_profile_quota(profile_name, &usage)),
            Err(err) => {
                let fields = vec![("Error".to_string(), first_line_of_error(&err.to_string()))];
                println!("{}", render_panel("Quota Watch", &fields));
            }
        }

        io::stdout()
            .flush()
            .context("failed to flush quota watch output")?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
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

fn read_usage_auth(codex_home: &Path) -> Result<UsageAuth> {
    let auth_path = codex_home.join("auth.json");
    if !auth_path.is_file() {
        bail!(
            "auth file not found at {}. Run `codex login` first.",
            auth_path.display()
        );
    }

    let content = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_path.display()))?;

    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        bail!("quota endpoint requires a ChatGPT access token. Run `codex login` first.");
    }

    let tokens = stored_auth
        .tokens
        .as_ref()
        .context("auth tokens are missing from auth.json")?;
    let access_token = tokens
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .context("access token not found in auth.json")?
        .to_string();
    let account_id = tokens
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(ToOwned::to_owned);

    Ok(UsageAuth {
        access_token,
        account_id,
    })
}

fn quota_base_url(explicit: Option<&str>) -> String {
    explicit
        .map(ToOwned::to_owned)
        .or_else(|| env::var("CODEX_CHATGPT_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_CHATGPT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn usage_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/usage")
    } else {
        format!("{base_url}/api/codex/usage")
    }
}

fn format_response_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| String::from_utf8_lossy(body).trim().to_string());
    }

    String::from_utf8_lossy(body).trim().to_string()
}

fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
) -> Vec<String> {
    ready_profile_candidates(
        &collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
        ),
        include_code_review,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect()
}

fn profile_rotation_order(state: &AppState, current_profile: &str) -> Vec<String> {
    let names: Vec<String> = state.profiles.keys().cloned().collect();
    let Some(index) = names.iter().position(|name| name == current_profile) else {
        return names
            .into_iter()
            .filter(|name| name != current_profile)
            .collect();
    };

    names
        .iter()
        .skip(index + 1)
        .chain(names.iter().take(index))
        .cloned()
        .collect()
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

fn shared_codex_root() -> Result<PathBuf> {
    match env::var_os("PRODEX_SHARED_CODEX_HOME") {
        Some(path) => absolutize(PathBuf::from(path)),
        None => default_codex_home(),
    }
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
            shared_codex_root: shared_codex_root()?,
            legacy_shared_codex_root: root.join("shared"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};

    fn usage_with_main_windows(
        five_hour_remaining: i64,
        five_hour_reset_offset_seconds: i64,
        weekly_remaining: i64,
        weekly_reset_offset_seconds: i64,
    ) -> UsageResponse {
        let now = Local::now().timestamp();
        UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                    reset_at: Some(now + five_hour_reset_offset_seconds),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                    reset_at: Some(now + weekly_reset_offset_seconds),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }
    }

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
        assert_eq!(blocked.len(), 2);
        assert!(blocked[0].message.starts_with("5h exhausted until "));
        assert_eq!(blocked[1].message, "weekly quota unavailable");
    }

    #[test]
    fn blocks_when_weekly_window_is_missing() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
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
        assert_eq!(blocked[0].message, "weekly quota unavailable");
    }

    #[test]
    fn compact_window_format_uses_scale_of_100() {
        let window = UsageWindow {
            used_percent: Some(37),
            reset_at: None,
            limit_window_seconds: Some(18_000),
        };

        assert_eq!(format_window_status_compact(&window), "5h 63% left");
        assert!(format_window_status(&window).contains("63% left"));
        assert!(format_window_status(&window).contains("37% used"));
    }

    #[test]
    fn main_reset_summary_lists_required_windows() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(30),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let summary = format_main_reset_summary(&usage);
        assert!(summary.starts_with("5h "));
        assert!(summary.contains(" | weekly "));
        assert!(summary.contains(&format_precise_reset_time(Some(1_700_000_000))));
    }

    #[test]
    fn main_reset_summary_marks_missing_required_window() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        assert_eq!(
            format_main_reset_summary(&usage),
            format!(
                "5h {} | weekly unavailable",
                format_precise_reset_time(Some(1_700_000_000))
            )
        );
    }

    #[test]
    fn map_parallel_runs_jobs_concurrently_and_preserves_order() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(Mutex::new(0usize));
        let started = Instant::now();

        let output = map_parallel(vec![1, 2, 3, 4], {
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            move |value| {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                {
                    let mut seen_max = max_active.lock().expect("max_active poisoned");
                    *seen_max = (*seen_max).max(current);
                }

                thread::sleep(Duration::from_millis(50));
                active.fetch_sub(1, Ordering::SeqCst);
                value * 10
            }
        });

        assert_eq!(output, vec![10, 20, 30, 40]);
        assert!(
            *max_active.lock().expect("max_active poisoned") >= 2,
            "parallel worker count never exceeded one"
        );
        assert!(
            started.elapsed() < Duration::from_millis(150),
            "parallel execution took too long: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn ready_profile_ranking_prefers_soon_recovering_weekly_capacity() {
        let candidates = vec![
            ReadyProfileCandidate {
                name: "slow".to_string(),
                usage: usage_with_main_windows(100, 18_000, 100, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "fast".to_string(),
                usage: usage_with_main_windows(80, 18_000, 80, 86_400),
                order_index: 1,
                preferred: false,
            },
        ];

        let mut ranked = candidates.clone();
        ranked.sort_by_key(ready_profile_sort_key);
        assert_eq!(ranked[0].name, "fast");
    }

    #[test]
    fn ready_profile_ranking_prefers_larger_reserve_when_resets_match() {
        let candidates = vec![
            ReadyProfileCandidate {
                name: "thin".to_string(),
                usage: usage_with_main_windows(65, 18_000, 70, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "deep".to_string(),
                usage: usage_with_main_windows(95, 18_000, 98, 604_800),
                order_index: 1,
                preferred: false,
            },
        ];

        let mut ranked = candidates.clone();
        ranked.sort_by_key(ready_profile_sort_key);
        assert_eq!(ranked[0].name, "deep");
    }

    #[test]
    fn quota_overview_sort_prioritizes_status_then_nearest_reset() {
        let reports = vec![
            QuotaReport {
                name: "blocked".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
            },
            QuotaReport {
                name: "ready-late".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
            },
            QuotaReport {
                name: "error".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("boom".to_string()),
            },
            QuotaReport {
                name: "ready-early".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
            },
        ];

        let names = sort_quota_reports_for_display(&reports)
            .into_iter()
            .map(|report| report.name.clone())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["ready-early", "ready-late", "blocked", "error"]);
    }

    #[test]
    fn rotates_profiles_after_current_profile() {
        let state = AppState {
            active_profile: Some("beta".to_string()),
            profiles: BTreeMap::from([
                (
                    "alpha".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/alpha"),
                        managed: true,
                        email: None,
                    },
                ),
                (
                    "beta".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/beta"),
                        managed: true,
                        email: None,
                    },
                ),
                (
                    "gamma".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/gamma"),
                        managed: true,
                        email: None,
                    },
                ),
            ]),
        };

        assert_eq!(
            profile_rotation_order(&state, "beta"),
            vec!["gamma".to_string(), "alpha".to_string()]
        );
    }

    #[test]
    fn backend_api_base_url_maps_to_wham_usage() {
        assert_eq!(
            usage_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham/usage"
        );
    }

    #[test]
    fn custom_base_url_maps_to_codex_usage() {
        assert_eq!(
            usage_url("http://127.0.0.1:8080"),
            "http://127.0.0.1:8080/api/codex/usage"
        );
    }

    #[test]
    fn profile_name_is_derived_from_email() {
        assert_eq!(
            profile_name_from_email("Main+Ops@Example.com"),
            "main-ops_example.com"
        );
    }

    #[test]
    fn unique_profile_name_adds_numeric_suffix() {
        let state = AppState {
            active_profile: None,
            profiles: BTreeMap::from([(
                "main_example.com".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/existing"),
                    managed: true,
                    email: Some("other@example.com".to_string()),
                },
            )]),
        };
        let paths = AppPaths {
            root: PathBuf::from("/tmp/prodex-test"),
            state_file: PathBuf::from("/tmp/prodex-test/state.json"),
            managed_profiles_root: PathBuf::from("/tmp/prodex-test/profiles"),
            shared_codex_root: PathBuf::from("/tmp/prodex-test/default-codex"),
            legacy_shared_codex_root: PathBuf::from("/tmp/prodex-test/shared"),
        };

        assert_eq!(
            unique_profile_name_for_email(&paths, &state, "main@example.com"),
            "main_example.com-2"
        );
    }

    #[test]
    fn parses_email_from_chatgpt_id_token() {
        let id_token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL3Byb2ZpbGUiOnsiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIn19.c2ln";

        assert_eq!(
            parse_email_from_id_token(id_token).expect("id token should parse"),
            Some("user@example.com".to_string())
        );
    }

    #[test]
    fn usage_response_accepts_null_additional_rate_limits() {
        let usage: UsageResponse = serde_json::from_value(serde_json::json!({
            "email": "user@example.com",
            "plan_type": "plus",
            "rate_limit": null,
            "code_review_rate_limit": null,
            "additional_rate_limits": null
        }))
        .expect("usage response should parse");

        assert!(usage.additional_rate_limits.is_empty());
    }

    #[test]
    fn section_headers_use_cli_width() {
        assert_eq!(text_width(&section_header("Quota Overview")), CLI_WIDTH);
    }

    #[test]
    fn field_lines_do_not_exceed_cli_width() {
        let fields = format_field_lines(
            "Path",
            "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered",
        );

        assert!(fields.iter().all(|line| text_width(line) <= CLI_WIDTH));
    }
}
