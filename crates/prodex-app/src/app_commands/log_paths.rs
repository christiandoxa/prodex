use anyhow::{Context, Result};
use prodex_core::AppPaths;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const SESSION_FOLLOW_LIMIT: usize = 32;

pub(crate) fn recent_session_log_paths() -> Result<Vec<PathBuf>> {
    let sessions_root = AppPaths::discover()?.shared_codex_root.join("sessions");
    let mut paths = Vec::new();
    collect_session_log_paths(&sessions_root, &mut paths)?;
    let observed = paths
        .into_iter()
        .map(|path| {
            let modified = path_modified_key(&path);
            (modified, path)
        })
        .collect::<Vec<_>>();
    Ok(order_recent_session_log_paths(
        observed,
        SESSION_FOLLOW_LIMIT,
    ))
}

/// The host keeps original PathBufs and metadata; the compiled log policy
/// decides the millisecond-precision order and the top-N limit.
fn order_recent_session_log_paths(paths: Vec<(u128, PathBuf)>, limit: usize) -> Vec<PathBuf> {
    use prodex_mojo_core::log::{
        RecentLogPathInput, RecentLogPathOrderMode, recent_log_path_order,
    };
    let inputs = paths
        .iter()
        .map(|(modified, path)| RecentLogPathInput {
            path: path.as_os_str().as_encoded_bytes(),
            modified_before_unix_epoch: false,
            modified_high: (*modified >> 64) as u64,
            modified_low: *modified as u64,
            modified_nanos: 0,
        })
        .collect::<Vec<_>>();
    let indices =
        recent_log_path_order(RecentLogPathOrderMode::SessionMilliseconds, &inputs, limit)
            .expect("compiled Mojo recent session log ordering returned invalid output");
    let mut original = paths
        .into_iter()
        .map(|(_, path)| Some(path))
        .collect::<Vec<_>>();
    indices
        .into_iter()
        .map(|index| {
            original[index]
                .take()
                .expect("validated Mojo log path indices are unique")
        })
        .collect()
}

fn collect_session_log_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", root.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_session_log_paths(&path, paths)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn path_modified_key(path: &Path) -> u128 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod compiled_recent_session_log_tests {
    use super::*;

    #[test]
    fn compiled_session_log_order_keeps_millisecond_ties_ascending_by_path() {
        let cases = vec![
            (2_500_u128, PathBuf::from("z-session.jsonl")),
            (2_500, PathBuf::from("雪-session.jsonl")),
            (2_500, PathBuf::from("a-session.jsonl")),
            (2_500, PathBuf::from("é-session.jsonl")),
            (1_000, PathBuf::from("old-session.jsonl")),
            (2_500, PathBuf::from("a-session.jsonl")),
        ];
        assert_eq!(
            order_recent_session_log_paths(cases.clone(), 4),
            vec![
                PathBuf::from("a-session.jsonl"),
                PathBuf::from("a-session.jsonl"),
                PathBuf::from("z-session.jsonl"),
                PathBuf::from("é-session.jsonl"),
            ],
        );
        assert_eq!(
            order_recent_session_log_paths(cases.clone(), 0),
            Vec::<PathBuf>::new(),
        );
        assert_eq!(
            order_recent_session_log_paths(cases, 32).last(),
            Some(&PathBuf::from("old-session.jsonl")),
        );
    }

    #[test]
    fn compiled_session_log_order_preserves_large_millisecond_epoch_and_io_acquisition() {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-order-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("run-雪.jsonl");
        std::fs::write(&path, b"session\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(UNIX_EPOCH + std::time::Duration::from_millis(12_345))
            .unwrap();
        let key = path_modified_key(&path);
        assert_eq!(key, 12_345);
        let other = PathBuf::from("far-future.jsonl");
        let selected = order_recent_session_log_paths(
            vec![
                (key, path.clone()),
                (u128::from(u64::MAX) + 10, other.clone()),
            ],
            1,
        );
        assert_eq!(selected, vec![other]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
