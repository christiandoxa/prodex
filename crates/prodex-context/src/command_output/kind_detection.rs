use super::log_stream::looks_like_log_stream_output;
use super::*;

#[cfg(any(not(feature = "mojo"), test))]
type MetadataCommandDetector = fn(&[String], usize, &str) -> Option<CommandOutputKind>;

#[cfg(any(not(feature = "mojo"), test))]
const METADATA_COMMAND_DETECTORS: [MetadataCommandDetector; 7] = [
    infer_metadata_build_command,
    infer_metadata_stream_command,
    infer_metadata_language_command,
    infer_metadata_cargo_command,
    infer_metadata_git_command,
    infer_metadata_docker_command,
    infer_metadata_package_command,
];

fn detect_command_output_kind(input: &str) -> CommandOutputKind {
    let lines = command_lines(input);
    detect_command_output_kind_from_lines(&lines)
}

fn detect_command_output_kind_from_lines(lines: &[&str]) -> CommandOutputKind {
    if let Some(kind) = detect_primary_command_output_kind(lines) {
        return kind;
    }
    if looks_like_git_status_lines(lines) {
        return CommandOutputKind::GitStatus;
    }
    if let Some(kind) = detect_search_command_output_kind(lines) {
        return kind;
    }
    if looks_like_file_list_output(lines) {
        return CommandOutputKind::FileList;
    }
    CommandOutputKind::Plain
}

fn detect_primary_command_output_kind(lines: &[&str]) -> Option<CommandOutputKind> {
    if looks_like_git_log_stat_output(lines) {
        return Some(CommandOutputKind::GitLog);
    }

    if looks_like_git_diff_output(lines) {
        return Some(CommandOutputKind::GitDiff);
    }

    if looks_like_rust_diagnostic_output(lines) {
        return Some(CommandOutputKind::RustDiagnostics);
    }

    if looks_like_noisy_success_output(lines) {
        return Some(CommandOutputKind::NoisySuccess);
    }

    if looks_like_log_stream_output(lines) {
        return Some(CommandOutputKind::LogStream);
    }

    if looks_like_diagnostic_output(lines) {
        return Some(CommandOutputKind::Diagnostics);
    }
    None
}

fn looks_like_git_status_lines(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        line.starts_with("On branch ")
            || line.starts_with("HEAD detached ")
            || line.starts_with("Changes to be committed:")
            || line.starts_with("Changes not staged for commit:")
            || line.starts_with("Untracked files:")
    }) || lines
        .iter()
        .filter(|line| is_short_git_status_line(line) || line.starts_with("## "))
        .take(3)
        .count()
        >= 2
}

fn detect_search_command_output_kind(lines: &[&str]) -> Option<CommandOutputKind> {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let search_matches = lines
        .iter()
        .filter(|line| {
            parse_search_match_line(line).is_some() || parse_rg_json_match_line(line).is_some()
        })
        .count();
    let heading_search_matches = count_heading_search_matches(lines);
    let rg_json_lines = lines
        .iter()
        .filter(|line| looks_like_rg_json_line(line))
        .count();
    let total_search_matches = search_matches.saturating_add(heading_search_matches);
    if total_search_matches >= 2 && total_search_matches.saturating_mul(2) >= non_empty {
        return Some(CommandOutputKind::Search);
    }
    if search_matches > 0 && rg_json_lines.saturating_mul(2) >= non_empty {
        return Some(CommandOutputKind::Search);
    }
    None
}

fn looks_like_file_list_output(lines: &[&str]) -> bool {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let file_list_lines = lines
        .iter()
        .filter(|line| parse_file_list_entry_line(line).is_some())
        .count();
    file_list_lines >= 4 && file_list_lines.saturating_mul(2) >= non_empty
}

pub(super) fn detect_command_output_kind_with_hint(
    input: &str,
    kind_hint: Option<CommandOutputKind>,
) -> CommandOutputKind {
    let detected = detect_command_output_kind(input);
    if detected == CommandOutputKind::Plain {
        kind_hint
            .filter(|kind| *kind != CommandOutputKind::Auto)
            .unwrap_or(detected)
    } else {
        detected
    }
}

#[cfg(not(feature = "mojo"))]
pub fn infer_command_output_kind_from_metadata(metadata: &str) -> Option<CommandOutputKind> {
    let tokens = command_metadata_tokens(metadata);
    infer_command_output_kind_from_metadata_tokens(&tokens)
}

#[cfg(feature = "mojo")]
pub fn infer_command_output_kind_from_metadata(metadata: &str) -> Option<CommandOutputKind> {
    let tag = prodex_mojo_core::context::classify_command_metadata(metadata)
        .unwrap_or_else(|error| panic!("Mojo command metadata classification failed: {error:?}"));
    command_output_kind_from_mojo_tag(tag)
}

fn command_output_kind_from_mojo_tag(tag: Option<i64>) -> Option<CommandOutputKind> {
    tag.and_then(|tag| match tag {
        1 => Some(CommandOutputKind::GitStatus),
        2 => Some(CommandOutputKind::GitDiff),
        3 => Some(CommandOutputKind::RustDiagnostics),
        4 => Some(CommandOutputKind::Diagnostics),
        5 => Some(CommandOutputKind::GitLog),
        6 => Some(CommandOutputKind::Search),
        7 => Some(CommandOutputKind::FileList),
        8 => Some(CommandOutputKind::LogStream),
        9 => Some(CommandOutputKind::NoisySuccess),
        _ => None,
    })
}

#[cfg(all(test, feature = "mojo"))]
pub(super) fn infer_command_output_kind_from_metadata_rust(
    metadata: &str,
) -> Option<CommandOutputKind> {
    let tokens = command_metadata_tokens(metadata);
    infer_command_output_kind_from_metadata_tokens(&tokens)
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_command_output_kind_from_metadata_tokens(tokens: &[String]) -> Option<CommandOutputKind> {
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        let inferred = infer_metadata_direct_command(command).or_else(|| {
            METADATA_COMMAND_DETECTORS
                .iter()
                .find_map(|detect| detect(tokens, index, command))
        });
        if inferred.is_some() {
            return inferred;
        }
    }
    None
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_direct_command(command: &str) -> Option<CommandOutputKind> {
    if matches!(command, "rg" | "ripgrep" | "grep" | "egrep" | "fgrep") {
        Some(CommandOutputKind::Search)
    } else if matches!(command, "ls" | "find" | "tree") {
        Some(CommandOutputKind::FileList)
    } else if matches!(
        command,
        "pytest"
            | "py.test"
            | "tsc"
            | "ruff"
            | "mypy"
            | "biome"
            | "oxlint"
            | "eslint"
            | "playwright"
            | "cypress"
    ) || command.ends_with("-tsc")
        || command.ends_with("_tsc")
    {
        Some(CommandOutputKind::Diagnostics)
    } else if matches!(
        command,
        "bazel"
            | "bazelisk"
            | "nx"
            | "turbo"
            | "pip"
            | "pip3"
            | "uv"
            | "nyc"
            | "c8"
            | "vite"
            | "next"
            | "docker-compose"
    ) {
        Some(CommandOutputKind::NoisySuccess)
    } else {
        None
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_build_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    let build_command = matches!(command, "gradle" | "gradlew")
        && command_metadata_subcommand_after(tokens, index)
            .is_some_and(|subcommand| matches!(subcommand, "test" | "check" | "build"))
        || matches!(command, "mvn" | "mvnw")
            && command_metadata_subcommand_after(tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "test" | "verify" | "package" | "install")
            });
    build_command.then_some(CommandOutputKind::NoisySuccess)
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_stream_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    (matches!(command, "journalctl" | "tail")
        || command == "kubectl" && command_metadata_subcommand_after(tokens, index) == Some("logs"))
    .then_some(CommandOutputKind::LogStream)
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_language_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    (command == "go"
        && command_metadata_subcommand_after(tokens, index)
            .is_some_and(|subcommand| matches!(subcommand, "vet" | "test" | "build")))
    .then_some(CommandOutputKind::Diagnostics)
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_cargo_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    if command != "cargo" {
        return None;
    }
    let subcommand = command_metadata_subcommand_after(tokens, index);
    if subcommand.is_some_and(|subcommand| {
        matches!(
            subcommand,
            "test" | "check" | "clippy" | "build" | "doc" | "nextest" | "fmt" | "fix"
        )
    }) {
        Some(CommandOutputKind::RustDiagnostics)
    } else if subcommand
        .is_some_and(|subcommand| matches!(subcommand, "update" | "install" | "fetch"))
    {
        Some(CommandOutputKind::NoisySuccess)
    } else {
        None
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_git_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    if command != "git" {
        return None;
    }
    match command_metadata_subcommand_after(tokens, index) {
        Some("status") => Some(CommandOutputKind::GitStatus),
        Some("diff" | "show") => Some(CommandOutputKind::GitDiff),
        Some("log") => Some(CommandOutputKind::GitLog),
        Some("grep") => Some(CommandOutputKind::Search),
        Some("ls-files") => Some(CommandOutputKind::FileList),
        _ => None,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_docker_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    if command != "docker" {
        return None;
    }
    let subcommand = command_metadata_subcommand_after(tokens, index);
    if subcommand == Some("compose")
        || subcommand.is_some_and(|subcommand| matches!(subcommand, "build" | "buildx" | "pull"))
    {
        Some(CommandOutputKind::NoisySuccess)
    } else {
        None
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn infer_metadata_package_command(
    tokens: &[String],
    index: usize,
    command: &str,
) -> Option<CommandOutputKind> {
    if !matches!(command, "npm" | "pnpm" | "yarn" | "bun") {
        return None;
    }
    if command_metadata_package_script_after(tokens, index).is_some() {
        Some(CommandOutputKind::Diagnostics)
    } else if command_metadata_package_install_after(tokens, index).is_some() {
        Some(CommandOutputKind::NoisySuccess)
    } else {
        None
    }
}

pub(super) fn command_metadata_subcommand_after(
    tokens: &[String],
    command_index: usize,
) -> Option<&str> {
    let mut skip_next = false;
    for token in tokens
        .iter()
        .skip(command_index + 1)
        .map(|token| command_metadata_token_command_name(token))
    {
        if skip_next {
            skip_next = false;
            continue;
        }
        if command_metadata_token_option_takes_value(token) {
            skip_next = true;
            continue;
        }
        if !command_metadata_token_is_option_or_shell_glue(token) {
            return Some(token);
        }
    }
    None
}

#[cfg(any(not(feature = "mojo"), test))]
fn command_metadata_package_script_after(tokens: &[String], command_index: usize) -> Option<&str> {
    let mut saw_run = false;
    let mut skip_next = false;
    for token in tokens
        .iter()
        .skip(command_index + 1)
        .map(|token| command_metadata_token_command_name(token))
    {
        if skip_next {
            skip_next = false;
            continue;
        }
        if command_metadata_token_option_takes_value(token) {
            skip_next = true;
            continue;
        }
        if command_metadata_token_is_option_or_shell_glue(token) {
            continue;
        }
        if token == "run" || token == "run-script" {
            saw_run = true;
            continue;
        }
        if matches!(
            token,
            "test" | "t" | "typecheck" | "type-check" | "tsc" | "check"
        ) || (saw_run && (token.contains("test") || token.contains("typecheck")))
        {
            return Some(token);
        }
        return None;
    }
    None
}

#[cfg(any(not(feature = "mojo"), test))]
fn command_metadata_package_install_after(tokens: &[String], command_index: usize) -> Option<&str> {
    let mut skip_next = false;
    for token in tokens
        .iter()
        .skip(command_index + 1)
        .map(|token| command_metadata_token_command_name(token))
    {
        if skip_next {
            skip_next = false;
            continue;
        }
        if command_metadata_token_option_takes_value(token) {
            skip_next = true;
            continue;
        }
        if command_metadata_token_is_option_or_shell_glue(token) {
            continue;
        }
        if matches!(
            token,
            "install" | "i" | "ci" | "add" | "update" | "upgrade" | "sync"
        ) {
            return Some(token);
        }
        return None;
    }
    None
}

fn command_metadata_token_is_option_or_shell_glue(token: &str) -> bool {
    token.is_empty()
        || token.starts_with('-')
        || token.starts_with('+')
        || matches!(
            token,
            "cmd"
                | "command"
                | "args"
                | "arguments"
                | "metadata"
                | "name"
                | "tool"
                | "tool_name"
                | "shell"
                | "bash"
                | "sh"
                | "zsh"
                | "fish"
                | "powershell"
                | "pwsh"
                | "python"
                | "python3"
                | "py"
                | "node"
                | "npx"
                | "bunx"
                | "uv"
                | "uvx"
                | "poetry"
                | "pipenv"
                | "exec_command"
                | "function_call"
                | "function_call_output"
                | "shell_call"
                | "shell_call_output"
                | "true"
                | "false"
                | "null"
        )
}

fn command_metadata_token_option_takes_value(token: &str) -> bool {
    matches!(
        token,
        "-c" | "-m"
            | "-p"
            | "--config"
            | "--git-dir"
            | "--work-tree"
            | "--manifest-path"
            | "--package"
            | "--bin"
            | "--example"
            | "--target"
            | "--project"
            | "--cwd"
            | "--prefix"
            | "--directory"
    )
}

pub(super) fn command_metadata_token_command_name(token: &str) -> &str {
    let basename = token.rsplit('/').next().unwrap_or(token);
    basename.strip_suffix(".exe").unwrap_or(basename)
}

pub(super) fn command_metadata_tokens(metadata: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    for ch in metadata.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '+') {
            token.push(ch.to_ascii_lowercase());
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_metadata_tests {
    use super::{
        infer_command_output_kind_from_metadata, infer_command_output_kind_from_metadata_rust,
    };

    #[test]
    fn mojo_metadata_classifier_matches_rust_oracle() {
        let cases = [
            "{\"cmd\":\"cargo test -q\"}",
            "command: cargo +nightly check --workspace",
            "rg --json needle crates",
            "grep -R needle src",
            "git -C repo status --short",
            "git diff --stat",
            "git log --stat --oneline",
            "pytest tests -q",
            "python -m pytest tests",
            "ruff check .",
            "mypy src",
            "biome check --write .",
            "oxlint --fix",
            "npx tsc --noEmit",
            "cargo clippy --fix --allow-dirty",
            "cargo fmt --all",
            "npm test -- --runInBand",
            "npm --prefix web run typecheck",
            "uv pip install -r requirements.txt",
            "bazel test //...",
            "npx nx affected -t build",
            "turbo run build",
            "docker compose up --wait",
            "kubectl logs deploy/prodex",
            "ls -la crates",
            "find crates -maxdepth 2 -type f",
            "tree -L 2 crates",
            "./gradlew test",
            "./mvnw verify",
            "/usr/bin/rg.exe needle src",
        ];

        for metadata in cases {
            assert_eq!(
                infer_command_output_kind_from_metadata(metadata),
                infer_command_output_kind_from_metadata_rust(metadata),
                "metadata: {metadata}"
            );
        }
    }

    #[test]
    fn mojo_metadata_classifier_bounds_untrusted_input() {
        let metadata = format!(
            "cargo check {}",
            "x".repeat(prodex_mojo_core::context::CONTEXT_METADATA_MAX_BYTES)
        );
        assert_eq!(infer_command_output_kind_from_metadata(&metadata), None);
    }
}
