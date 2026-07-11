use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const LOG_FOLLOW_READ_CHUNK_BYTES: usize = 1024 * 1024;
const LOG_FOLLOW_PENDING_MAX_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub(crate) struct FollowedLog {
    pub(crate) offset: u64,
    pub(crate) pending: String,
}

pub(crate) fn collect_new_followed_lines(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<String>> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("failed to open {}", path.display())),
    };
    let len = file.metadata()?.len();
    if len < state.offset {
        state.offset = 0;
        state.pending.clear();
    }
    file.seek(SeekFrom::Start(state.offset))?;
    let mut bytes = Vec::new();
    file.take(LOG_FOLLOW_READ_CHUNK_BYTES as u64)
        .read_to_end(&mut bytes)?;
    state.offset = state.offset.saturating_add(bytes.len() as u64);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    state.pending.push_str(&String::from_utf8_lossy(&bytes));
    let complete_len = state
        .pending
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or_default();
    if complete_len == 0 {
        if state.pending.len() > LOG_FOLLOW_PENDING_MAX_BYTES {
            state.pending.clear();
        }
        return Ok(Vec::new());
    }
    let complete = state.pending[..complete_len].to_string();
    state.pending.drain(..complete_len);
    if state.pending.len() > LOG_FOLLOW_PENDING_MAX_BYTES {
        state.pending.clear();
    }
    Ok(complete.lines().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn followed_log_drops_oversized_partial_line() {
        let root = std::env::temp_dir().join(format!(
            "prodex-log-follow-large-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("runtime.log");
        fs::write(&path, vec![b'a'; LOG_FOLLOW_READ_CHUNK_BYTES + 1]).unwrap();
        let mut state = FollowedLog::default();

        assert!(
            collect_new_followed_lines(&path, &mut state)
                .unwrap()
                .is_empty()
        );
        assert_eq!(state.pending.len(), LOG_FOLLOW_PENDING_MAX_BYTES);
        assert!(
            collect_new_followed_lines(&path, &mut state)
                .unwrap()
                .is_empty()
        );
        assert!(state.pending.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
