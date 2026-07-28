use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const LOG_FOLLOW_READ_CHUNK_BYTES: usize = 1024 * 1024;
const LOG_FOLLOW_PENDING_MAX_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub(crate) struct FollowedLog {
    pub(crate) offset: u64,
    pub(crate) pending: Vec<u8>,
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

    state.pending.extend_from_slice(&bytes);
    let complete_len = state
        .pending
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or_default();
    if complete_len == 0 {
        if state.pending.len() > LOG_FOLLOW_PENDING_MAX_BYTES {
            state.pending.clear();
        }
        return Ok(Vec::new());
    }
    let complete = String::from_utf8_lossy(&state.pending[..complete_len]).into_owned();
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
    fn followed_log_bounds_partial_lines_and_preserves_split_utf8() {
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

        let mut content = vec![b'a'; LOG_FOLLOW_READ_CHUNK_BYTES - 2];
        content.extend_from_slice("🌋\n".as_bytes());
        fs::write(&path, content).unwrap();
        let mut state = FollowedLog::default();
        assert!(
            collect_new_followed_lines(&path, &mut state)
                .unwrap()
                .is_empty()
        );
        let lines = collect_new_followed_lines(&path, &mut state).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('🌋'));
        assert!(!lines[0].contains('\u{fffd}'));
        fs::remove_dir_all(root).unwrap();
    }
}
