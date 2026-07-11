use super::*;

pub fn compact_successful_command_output_with_options(
    input: &str,
    options: &CommandSuccessOutputCompactOptions,
) -> CommandSuccessOutputCompactReport {
    let normalized = normalize_command_output(input);
    let lines = command_lines(&normalized);
    let original_lines = count_text_lines(&normalized);
    let critical_signals = count_critical_signals(&normalized);
    let failure_suspected =
        command_success_output_failure_suspected(&lines, critical_signals, options);

    if normalized.is_empty() || failure_suspected {
        return CommandSuccessOutputCompactReport {
            compacted: false,
            failure_suspected,
            original_lines,
            compacted_lines: original_lines,
            touched_files: collect_success_output_touched_files(&lines).len(),
            critical_signals,
            output: normalized,
        };
    }

    let success_like = command_success_output_success_like(&lines, options);
    if !success_like || original_lines < options.min_lines_to_compact.max(1) {
        return CommandSuccessOutputCompactReport {
            compacted: false,
            failure_suspected: false,
            original_lines,
            compacted_lines: original_lines,
            touched_files: collect_success_output_touched_files(&lines).len(),
            critical_signals,
            output: normalized,
        };
    }

    let touched_files = collect_success_output_touched_files(&lines);
    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    for line in &lines {
        if let Some(label) = noisy_success_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
        }
        if is_noisy_success_key_line(line) || is_rust_success_summary_line(line) {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
        }
    }

    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let noisy_success_lines = noise_counts.values().sum::<usize>();
    let mut output = Vec::<String>::new();
    output.push(format!("pcs: success cmd ({}->sum)", original_lines));
    output.push(format!(
        "command: {}",
        options.command.as_deref().unwrap_or("(unknown)")
    ));
    output.push(format!(
        "exit: {}",
        options
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "(unknown)".to_string())
    ));
    output.push(format!(
        "counts: lines={}, non_empty={}, noisy_success_lines={}, critical_signals={}, touched_files={}",
        original_lines,
        non_empty,
        noisy_success_lines,
        critical_signals.total(),
        touched_files.len()
    ));
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }

    let include_path_summary =
        command_success_output_path_summary_useful(&lines, &touched_files, options);
    if include_path_summary {
        let roots = count_success_output_path_roots(&touched_files);
        if !roots.is_empty() {
            output.push(format_count_map("top roots", &roots, 8));
        }
        let extensions = count_success_output_path_extensions(&touched_files);
        if !extensions.is_empty() {
            output.push(format_count_map("extensions", &extensions, 8));
        }
    }

    if command_success_output_can_use_short_success_summary(
        critical_signals,
        include_path_summary,
        touched_files.len(),
        &key_lines,
        options,
    ) {
        let mut short = format!(
            "pcs: ok lines={} noisy={} touched={}",
            original_lines,
            noisy_success_lines,
            touched_files.len()
        );
        if !noise_counts.is_empty() {
            short.push_str(" | ");
            short.push_str(&format_count_map("noise", &noise_counts, 6));
        }
        if let Some(command) = options
            .command
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            short.push_str(" | ");
            short.push_str(&format!(
                "cmd: {}",
                truncate_command_line(command, options.max_line_chars)
            ));
        }
        let output = lines_to_text(vec![short]);
        return CommandSuccessOutputCompactReport {
            compacted: true,
            failure_suspected: false,
            original_lines,
            compacted_lines: count_text_lines(&output),
            touched_files: touched_files.len(),
            critical_signals,
            output,
        };
    }

    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_key_lines.max(1),
    );
    if include_path_summary {
        push_success_output_touched_files(&mut output, &touched_files, options);
    }

    let output = lines_to_text(output);
    let output =
        canonicalize_compacted_command_paths(&normalized, &output, CommandOutputKind::NoisySuccess);
    CommandSuccessOutputCompactReport {
        compacted: true,
        failure_suspected: false,
        original_lines,
        compacted_lines: count_text_lines(&output),
        touched_files: touched_files.len(),
        critical_signals,
        output,
    }
}

fn command_success_output_can_use_short_success_summary(
    critical_signals: CriticalSignalCounts,
    include_path_summary: bool,
    touched_files: usize,
    key_lines: &[String],
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    let short_candidate = options
        .command
        .as_deref()
        .is_some_and(command_name_is_short_success_output_candidate);

    if critical_signals.total() != 0
        || include_path_summary && !short_candidate
        || !key_lines
            .iter()
            .all(|line| noisy_success_label(line).is_some() || is_rust_success_summary_line(line))
    {
        return false;
    }

    touched_files == 0 || short_candidate
}

fn command_success_output_failure_suspected(
    lines: &[&str],
    critical_signals: CriticalSignalCounts,
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    options.exit_code.is_some_and(|code| code != 0)
        || !critical_signals.is_empty()
        || lines.iter().any(|line| {
            is_error_signal_line(line)
                || is_test_failure_signal_line(line)
                || is_success_output_failure_signal_line(line)
                || is_success_output_warning_signal_line(line)
                || is_diagnostic_failure_summary_line(line)
        })
}

fn command_success_output_success_like(
    lines: &[&str],
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    options.exit_code == Some(0)
        || looks_like_noisy_success_output(lines)
        || options
            .command
            .as_deref()
            .is_some_and(command_name_is_success_output_candidate)
}

fn command_name_is_success_output_candidate(command: &str) -> bool {
    let tokens = command_metadata_tokens(command);
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(
            command,
            "cargo"
                | "npm"
                | "pnpm"
                | "yarn"
                | "bun"
                | "corepack"
                | "make"
                | "cmake"
                | "ninja"
                | "ls"
                | "find"
                | "tree"
                | "du"
                | "tar"
                | "unzip"
                | "pip"
                | "pip3"
                | "uv"
                | "pipenv"
                | "poetry"
                | "ruff"
                | "mypy"
                | "biome"
                | "oxlint"
                | "pytest"
                | "py.test"
                | "swift"
                | "zig"
                | "tsc"
                | "vitest"
                | "jest"
                | "eslint"
                | "prettier"
                | "vite"
                | "next"
                | "playwright"
                | "cypress"
                | "nyc"
                | "c8"
                | "mvn"
                | "mvnw"
                | "gradle"
                | "gradlew"
                | "bazel"
                | "bazelisk"
                | "nx"
                | "turbo"
                | "docker-compose"
        ) {
            return true;
        }
        if command.ends_with("-tsc") || command.ends_with("_tsc") {
            return true;
        }
        if command == "go"
            && command_metadata_subcommand_after(&tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "test" | "build" | "vet" | "list"))
        {
            return true;
        }
        if command == "docker"
            && command_metadata_subcommand_after(&tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "build" | "buildx" | "pull" | "compose")
            })
        {
            return true;
        }
    }
    false
}

fn command_name_is_short_success_output_candidate(command: &str) -> bool {
    if command.contains("&&") || command.contains(';') || command.contains('|') {
        return false;
    }

    let tokens = command_metadata_tokens(command);
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(
            command,
            "tsc"
                | "vitest"
                | "jest"
                | "vite"
                | "next"
                | "playwright"
                | "cypress"
                | "biome"
                | "oxlint"
                | "pnpm"
        ) || command.ends_with("-tsc")
            || command.ends_with("_tsc")
        {
            return true;
        }
        if command == "cargo"
            && command_metadata_subcommand_after(&tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "clippy" | "doc" | "fmt" | "fix" | "nextest")
            })
        {
            return true;
        }
        if matches!(command, "bun" | "swift" | "zig")
            || command == "uv"
                && command_metadata_subcommand_after(&tokens, index) == Some("run")
                && tokens
                    .iter()
                    .skip(index + 2)
                    .map(|token| command_metadata_token_command_name(token))
                    .any(|token| matches!(token, "pytest" | "py.test"))
        {
            return true;
        }
    }
    false
}

fn command_success_output_path_summary_useful(
    lines: &[&str],
    paths: &[String],
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    !paths.is_empty()
        && (options
            .command
            .as_deref()
            .is_some_and(command_name_is_path_relevant_success_output)
            || lines.iter().any(|line| {
                parse_file_list_entry_line(line).is_some()
                    || parse_search_match_line(line).is_some()
                    || parse_rg_json_match_line(line).is_some()
            }))
}

fn command_name_is_path_relevant_success_output(command: &str) -> bool {
    let tokens = command_metadata_tokens(command);
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(command, "ls" | "find" | "tree" | "du" | "rg" | "grep") {
            return true;
        }
        if command == "go"
            && command_metadata_subcommand_after(&tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "list"))
        {
            return true;
        }
    }
    false
}

fn collect_success_output_touched_files(lines: &[&str]) -> Vec<String> {
    let mut paths = BTreeMap::<String, ()>::new();
    for line in lines {
        if let Some(path) = parse_file_list_entry_line(line) {
            insert_success_output_path(&mut paths, &path);
        }
        if let Some(search_match) =
            parse_search_match_line(line).or_else(|| parse_rg_json_match_line(line))
        {
            insert_success_output_path(&mut paths, &search_match.path);
        }
        for token in line.split_whitespace() {
            if let Some(path) = success_output_path_from_token(token) {
                insert_success_output_path(&mut paths, &path);
            }
        }
    }
    paths.into_keys().collect()
}

fn insert_success_output_path(paths: &mut BTreeMap<String, ()>, path: &str) {
    let normalized = path.replace('\\', "/");
    let normalized = context_noise_strip_path_location_suffix_supplement(&normalized)
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
            )
        })
        .trim_matches('/')
        .to_string();
    if normalized.len() < 3 || normalized.contains("://") || normalized.contains(' ') {
        return;
    }
    if normalized.starts_with('-') || normalized == "." || normalized == ".." {
        return;
    }
    paths.insert(normalized, ());
}

fn success_output_path_from_token(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
    });
    context_noise_normalize_path_token_supplement(token).or_else(|| {
        let normalized = context_noise_strip_path_location_suffix_supplement(token);
        looks_like_bare_path_entry(normalized).then(|| normalized.to_string())
    })
}

pub(super) fn count_success_output_path_roots(paths: &[String]) -> BTreeMap<String, usize> {
    let mut roots = BTreeMap::<String, usize>::new();
    for path in paths {
        *roots.entry(top_level_path_segment(path)).or_default() += 1;
    }
    roots
}

pub(super) fn count_success_output_path_extensions(paths: &[String]) -> BTreeMap<String, usize> {
    let mut extensions = BTreeMap::<String, usize>::new();
    for path in paths {
        *extensions.entry(path_extension_label(path)).or_default() += 1;
    }
    extensions
}

fn push_success_output_touched_files(
    output: &mut Vec<String>,
    paths: &[String],
    options: &CommandSuccessOutputCompactOptions,
) {
    if paths.is_empty() {
        return;
    }
    let limit = options.max_touched_files.max(1);
    output.push(format!("touched files ({}):", paths.len()));
    for path in paths.iter().take(limit) {
        output.push(format!(
            "  {}",
            truncate_command_line(path, options.max_line_chars)
        ));
    }
    if paths.len() > limit {
        output.push(format!(
            "  [... {} more touched files ...]",
            paths.len() - limit
        ));
    }
}
