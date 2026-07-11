use super::log_stream::looks_like_log_stream_output;
use super::*;

fn detect_command_output_kind(input: &str) -> CommandOutputKind {
    let lines = command_lines(input);
    if looks_like_git_log_stat_output(&lines) {
        return CommandOutputKind::GitLog;
    }

    if looks_like_git_diff_output(&lines) {
        return CommandOutputKind::GitDiff;
    }

    if looks_like_rust_diagnostic_output(&lines) {
        return CommandOutputKind::RustDiagnostics;
    }

    if looks_like_noisy_success_output(&lines) {
        return CommandOutputKind::NoisySuccess;
    }

    if looks_like_log_stream_output(&lines) {
        return CommandOutputKind::LogStream;
    }

    if looks_like_diagnostic_output(&lines) {
        return CommandOutputKind::Diagnostics;
    }

    if lines.iter().any(|line| {
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
    {
        return CommandOutputKind::GitStatus;
    }

    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let search_matches = lines
        .iter()
        .filter(|line| {
            parse_search_match_line(line).is_some() || parse_rg_json_match_line(line).is_some()
        })
        .count();
    let heading_search_matches = count_heading_search_matches(&lines);
    let rg_json_lines = lines
        .iter()
        .filter(|line| looks_like_rg_json_line(line))
        .count();
    let total_search_matches = search_matches.saturating_add(heading_search_matches);
    if total_search_matches >= 2 && total_search_matches.saturating_mul(2) >= non_empty {
        return CommandOutputKind::Search;
    }
    if search_matches > 0 && rg_json_lines.saturating_mul(2) >= non_empty {
        return CommandOutputKind::Search;
    }

    let file_list_lines = lines
        .iter()
        .filter(|line| parse_file_list_entry_line(line).is_some())
        .count();
    if file_list_lines >= 4 && file_list_lines.saturating_mul(2) >= non_empty {
        return CommandOutputKind::FileList;
    }

    CommandOutputKind::Plain
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

pub fn infer_command_output_kind_from_metadata(metadata: &str) -> Option<CommandOutputKind> {
    let tokens = command_metadata_tokens(metadata);
    infer_command_output_kind_from_metadata_tokens(&tokens)
}

fn infer_command_output_kind_from_metadata_tokens(tokens: &[String]) -> Option<CommandOutputKind> {
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(command, "rg" | "ripgrep" | "grep" | "egrep" | "fgrep") {
            return Some(CommandOutputKind::Search);
        }
        if matches!(command, "ls" | "find" | "tree") {
            return Some(CommandOutputKind::FileList);
        }
        if matches!(
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
            return Some(CommandOutputKind::Diagnostics);
        }
        if matches!(
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
        ) {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "gradle" | "gradlew")
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "test" | "check" | "build"))
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "mvn" | "mvnw")
            && command_metadata_subcommand_after(tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "test" | "verify" | "package" | "install")
            })
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "journalctl" | "tail")
            || command == "kubectl"
                && command_metadata_subcommand_after(tokens, index) == Some("logs")
        {
            return Some(CommandOutputKind::LogStream);
        }
        if command == "go"
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "vet" | "test" | "build"))
        {
            return Some(CommandOutputKind::Diagnostics);
        }
        if command == "cargo"
            && command_metadata_subcommand_after(tokens, index).is_some_and(|subcommand| {
                matches!(
                    subcommand,
                    "test" | "check" | "clippy" | "build" | "doc" | "nextest" | "fmt" | "fix"
                )
            })
        {
            return Some(CommandOutputKind::RustDiagnostics);
        }
        if command == "cargo"
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "update" | "install" | "fetch"))
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if command == "git"
            && let Some(subcommand) = command_metadata_subcommand_after(tokens, index)
        {
            match subcommand {
                "status" => return Some(CommandOutputKind::GitStatus),
                "diff" | "show" => return Some(CommandOutputKind::GitDiff),
                "log" => return Some(CommandOutputKind::GitLog),
                "grep" => return Some(CommandOutputKind::Search),
                "ls-files" => return Some(CommandOutputKind::FileList),
                _ => {}
            }
        }
        if command == "docker"
            && command_metadata_subcommand_after(tokens, index) == Some("compose")
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if command == "docker"
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "build" | "buildx" | "pull"))
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "npm" | "pnpm" | "yarn" | "bun")
            && command_metadata_package_script_after(tokens, index).is_some()
        {
            return Some(CommandOutputKind::Diagnostics);
        }
        if matches!(command, "npm" | "pnpm" | "yarn" | "bun")
            && command_metadata_package_install_after(tokens, index).is_some()
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "docker-compose") {
            return Some(CommandOutputKind::NoisySuccess);
        }
    }
    None
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
