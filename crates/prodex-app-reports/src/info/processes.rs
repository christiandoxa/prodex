use super::*;

pub fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            continue;
        }
        let Ok(pid) = tokens[0].parse::<u32>() else {
            continue;
        };
        rows.push(ProcessRow {
            pid,
            command: tokens[1].to_string(),
            args: tokens
                .iter()
                .skip(2)
                .map(|token| (*token).to_string())
                .collect(),
        });
    }
    rows
}

pub fn classify_prodex_process_row(
    row: ProcessRow,
    current_pid: u32,
    current_basename: Option<&str>,
) -> Option<ProdexProcessInfo> {
    if row.pid == current_pid || !is_prodex_process_row(&row, current_basename) {
        return None;
    }

    Some(ProdexProcessInfo {
        pid: row.pid,
        runtime: prodex_process_row_is_runtime(&row, current_basename),
    })
}

pub fn is_prodex_process_row(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    let command_base = process_basename(&row.command);
    process_basename_matches(command_base, current_basename)
        || prodex_process_row_argv_span(row, current_basename).is_some()
}

pub fn prodex_process_row_is_runtime(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    prodex_process_row_argv_span(row, current_basename)
        .and_then(|args| args.get(1))
        .is_some_and(|arg| arg == "run" || arg == "__runtime-broker")
}

pub fn prodex_process_row_argv_span<'a>(
    row: &'a ProcessRow,
    current_basename: Option<&str>,
) -> Option<&'a [String]> {
    row.args.iter().enumerate().find_map(|(index, arg)| {
        process_basename_matches(process_basename(arg), current_basename)
            .then_some(&row.args[index..])
    })
}

pub fn process_basename_matches(candidate: &str, current_basename: Option<&str>) -> bool {
    candidate == "prodex" || current_basename.is_some_and(|name| candidate == name)
}

pub fn process_basename(input: &str) -> &str {
    Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

pub fn runtime_log_pid_from_path(path: &Path) -> Option<u32> {
    runtime_log_pid_from_path_with_prefix(path, "prodex-runtime")
}

pub fn runtime_log_pid_from_path_with_prefix(path: &Path, prefix: &str) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{prefix}-"))?;
    let (pid, _) = rest.split_once('-')?;
    pid.parse::<u32>().ok()
}

pub fn select_active_runtime_log_paths<I>(
    processes: &[ProdexProcessInfo],
    log_paths: I,
) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    select_active_runtime_log_paths_with_prefix(processes, log_paths, "prodex-runtime")
}

pub fn select_active_runtime_log_paths_with_prefix<I>(
    processes: &[ProdexProcessInfo],
    log_paths: I,
    prefix: &str,
) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let runtime_pids = processes
        .iter()
        .filter(|process| process.runtime)
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    if runtime_pids.is_empty() {
        return Vec::new();
    }

    let mut latest_logs = BTreeMap::new();
    for path in log_paths {
        let Some(pid) = runtime_log_pid_from_path_with_prefix(&path, prefix) else {
            continue;
        };
        if runtime_pids.contains(&pid) {
            latest_logs.insert(pid, path);
        }
    }
    latest_logs.into_values().collect()
}

pub fn select_recent_runtime_log_paths<I>(log_paths: I, limit: usize) -> Vec<PathBuf>
where
    I: IntoIterator<Item = (PathBuf, SystemTime)>,
{
    let mut paths = log_paths.into_iter().collect::<Vec<_>>();
    paths.sort_by(|(left_path, left_modified), (right_path, right_modified)| {
        left_modified
            .cmp(right_modified)
            .then_with(|| left_path.cmp(right_path))
    });
    paths.reverse();
    paths.truncate(limit);
    paths.into_iter().map(|(path, _)| path).collect()
}
