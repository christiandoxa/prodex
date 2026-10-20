//! Process classification and runtime-log selection.

use super::{BTreeMap, BTreeSet, Path, PathBuf, ProcessRow, ProdexProcessInfo, SystemTime};
use prodex_cli::Commands;

pub fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
    let (row_spans, token_spans) = prodex_mojo_core::status_resource::parse_process_rows(text)
        .expect("Mojo process-row parser returned invalid output");
    row_spans
        .into_iter()
        .map(|row| {
            let tokens = &token_spans[row.token_start..row.token_start + row.token_count];
            ProcessRow {
                pid: row.pid,
                command: text[tokens[1].start..tokens[1].end].to_string(),
                args: tokens[2..]
                    .iter()
                    .map(|token| text[token.start..token.end].to_string())
                    .collect(),
            }
        })
        .collect()
}

pub fn classify_prodex_process_row(
    row: ProcessRow,
    current_pid: u32,
    current_basename: Option<&str>,
) -> Option<ProdexProcessInfo> {
    if row.pid == current_pid || !is_prodex_process_row(&row, current_basename) {
        return None;
    }

    let command = prodex_process_row_command(&row, current_basename);
    Some(ProdexProcessInfo {
        pid: row.pid,
        command: command
            .as_ref()
            .map(Commands::process_label)
            .unwrap_or("unknown")
            .to_string(),
        runtime: command.as_ref().is_some_and(Commands::launches_runtime),
    })
}

pub fn is_prodex_process_row(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    let command_base = process_basename(&row.command);
    process_basename_matches(command_base, current_basename)
        || prodex_process_row_argv_span(row, current_basename).is_some()
}

fn prodex_process_row_command(
    row: &ProcessRow,
    current_basename: Option<&str>,
) -> Option<Commands> {
    let args = prodex_process_row_argv_span(row, current_basename)?;
    prodex_cli::parse_cli_command_from(args.iter().cloned()).ok()
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

pub fn runtime_log_pid_from_path_with_prefix(path: &Path, prefix: &str) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{prefix}-"))?;
    let (pid, _) = rest.split_once('-')?;
    pid.parse::<u32>().ok()
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
    use prodex_mojo_core::log::{
        RecentLogPathInput, RecentLogPathOrderMode, recent_log_path_order,
    };
    let paths = log_paths.into_iter().collect::<Vec<_>>();
    let facts = paths
        .iter()
        .map(|(path, modified)| {
            let (before_epoch, absolute) = match modified.duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => (false, duration),
                Err(error) => (true, error.duration()),
            };
            RecentLogPathInput {
                path: path.as_os_str().as_encoded_bytes(),
                modified_before_unix_epoch: before_epoch,
                modified_high: 0,
                modified_low: absolute.as_secs(),
                modified_nanos: absolute.subsec_nanos(),
            }
        })
        .collect::<Vec<_>>();
    let indices = recent_log_path_order(RecentLogPathOrderMode::RuntimeSystemTime, &facts, limit)
        .expect("compiled Mojo recent runtime-info log ordering returned invalid output");
    let mut paths = paths
        .into_iter()
        .map(|(path, _)| Some(path))
        .collect::<Vec<_>>();
    indices
        .into_iter()
        .map(|index| {
            paths[index]
                .take()
                .expect("validated Mojo log path indices are unique")
        })
        .collect()
}

#[cfg(test)]
mod compiled_recent_runtime_log_tests {
    use super::*;

    #[test]
    fn compiled_runtime_info_log_order_preserves_subsecond_pre_epoch_and_unicode_path_ties() {
        let base = std::time::UNIX_EPOCH;
        let newer = base + std::time::Duration::from_secs(6);
        let same = base + std::time::Duration::from_secs(5);
        let newer_nanos = same + std::time::Duration::from_nanos(1);
        let past = base - std::time::Duration::from_nanos(1);
        let ancient = base - std::time::Duration::from_secs(2);
        let selected = select_recent_runtime_log_paths(
            [
                (PathBuf::from("a.log"), same),
                (PathBuf::from("z.log"), same),
                (PathBuf::from("雪.log"), same),
                (PathBuf::from("newest.log"), newer),
                (PathBuf::from("subsecond.log"), newer_nanos),
                (PathBuf::from("past.log"), past),
                (PathBuf::from("ancient.log"), ancient),
            ],
            6,
        );
        assert_eq!(
            selected,
            [
                "newest.log",
                "subsecond.log",
                "雪.log",
                "z.log",
                "a.log",
                "past.log",
            ]
            .map(PathBuf::from)
            .to_vec(),
        );
        assert_eq!(
            select_recent_runtime_log_paths(
                [
                    (PathBuf::from("a.log"), same),
                    (PathBuf::from("z.log"), same),
                ],
                0,
            ),
            Vec::<PathBuf>::new(),
        );
    }
}
