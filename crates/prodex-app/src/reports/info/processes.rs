use super::{PathBuf, ProcessRow, SystemTime};

pub fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
    text.lines()
        .filter_map(|line| {
            let tokens = line.split_whitespace().collect::<Vec<_>>();
            if tokens.len() < 2 {
                return None;
            }
            Some(ProcessRow {
                pid: tokens[0].parse().ok()?,
                command: tokens[1].to_string(),
                args: tokens
                    .iter()
                    .skip(2)
                    .map(|token| (*token).to_string())
                    .collect(),
            })
        })
        .collect()
}

pub fn select_recent_runtime_log_paths<I>(log_paths: I, limit: usize) -> Vec<PathBuf>
where
    I: IntoIterator<Item = (PathBuf, SystemTime)>,
{
    let mut paths = log_paths.into_iter().collect::<Vec<_>>();
    paths.sort_by(|(left_path, left_modified), (right_path, right_modified)| {
        right_modified
            .cmp(left_modified)
            .then_with(|| right_path.cmp(left_path))
    });
    paths.truncate(limit);
    paths.into_iter().map(|(path, _)| path).collect()
}
