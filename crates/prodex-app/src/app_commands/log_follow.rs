use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::time::SystemTime;
use std::time::{Duration, Instant};

const LOG_FOLLOW_READ_CHUNK_BYTES: usize = 1024 * 1024;
const LOG_FOLLOW_PENDING_MAX_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub(crate) struct FollowedLog {
    pub(crate) offset: u64,
    pub(crate) pending: Vec<u8>,
    file: Option<fs::File>,
    file_identity: Option<FileIdentity>,
    #[cfg(windows)]
    path_modified: Option<SystemTime>,
}

impl FollowedLog {
    pub(crate) fn with_offset(offset: u64) -> Self {
        Self {
            offset,
            ..Self::default()
        }
    }

    pub(crate) fn at_end(path: &Path) -> Self {
        Self::with_offset(
            fs::metadata(path)
                .map(|metadata| metadata.len())
                .unwrap_or_default(),
        )
    }
}

pub(crate) struct FollowedLogPaths {
    paths: Vec<PathBuf>,
    refresh_interval: Duration,
    last_refresh: Option<Instant>,
}

pub(crate) fn retain_followed_logs(
    followed: &mut BTreeMap<PathBuf, FollowedLog>,
    current_paths: &[PathBuf],
) {
    followed.retain(|path, _| current_paths.contains(path));
}

impl Default for FollowedLogPaths {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl FollowedLogPaths {
    pub(crate) fn new(paths: Vec<PathBuf>) -> Self {
        Self::with_refresh_interval(paths, Duration::from_secs(2))
    }

    pub(crate) fn with_refresh_interval(paths: Vec<PathBuf>, refresh_interval: Duration) -> Self {
        Self {
            paths,
            refresh_interval,
            last_refresh: Some(Instant::now()),
        }
    }

    pub(crate) fn refresh(&mut self, discover: impl FnOnce() -> Vec<PathBuf>) -> &[PathBuf] {
        if self
            .last_refresh
            .is_none_or(|last_refresh| last_refresh.elapsed() >= self.refresh_interval)
        {
            self.paths = discover();
            self.last_refresh = Some(Instant::now());
        }
        &self.paths
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    first: u64,
    second: u64,
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Option<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(FileIdentity {
        first: metadata.dev(),
        second: metadata.ino(),
    })
}

#[cfg(windows)]
fn file_identity(file: &fs::File) -> Option<FileIdentity> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: `file` owns a live handle and `information` points to writable
    // storage for the exact structure required by the Windows API.
    let result = unsafe {
        GetFileInformationByHandle(file.as_raw_handle().cast(), information.as_mut_ptr())
    };
    (result != 0).then(|| {
        // SAFETY: the successful API call initialized the complete structure.
        let information = unsafe { information.assume_init() };
        FileIdentity {
            first: u64::from(information.dwVolumeSerialNumber),
            second: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        }
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_metadata: &fs::Metadata) -> Option<FileIdentity> {
    None
}

pub(crate) fn collect_new_followed_lines(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<String>> {
    let path_metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            state.file = None;
            state.file_identity = None;
            return Ok(Vec::new());
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            state.file = None;
            state.file_identity = None;
            return Ok(Vec::new());
        }
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };

    #[cfg(windows)]
    let mut path_file = {
        let modified = path_metadata.modified().ok();
        (state.file.is_none() || state.path_modified != modified)
            .then(|| {
                fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))
            })
            .transpose()?
    };
    #[cfg(not(windows))]
    let mut path_file = None;

    #[cfg(windows)]
    let path_identity = path_file.as_ref().and_then(file_identity);
    #[cfg(not(windows))]
    let path_identity = file_identity(&path_metadata);
    let replace_file = match path_identity {
        Some(identity) => state.file_identity != Some(identity),
        None => state.file.is_none(),
    };
    if replace_file {
        let had_file = state.file.is_some();
        state.file = Some(path_file.take().unwrap_or(
            fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
        ));
        state.file_identity = path_identity;
        if had_file {
            state.offset = 0;
            state.pending.clear();
        }
    }
    #[cfg(windows)]
    {
        state.path_modified = path_metadata.modified().ok();
    }

    let file_len = path_metadata.len();
    if file_len < state.offset {
        state.offset = 0;
        state.pending.clear();
    }
    if file_len == state.offset {
        return Ok(Vec::new());
    }
    let file = state
        .file
        .as_mut()
        .context("followed log file was not opened")?;
    file.seek(SeekFrom::Start(state.offset))?;
    let pending_len = state.pending.len();
    (&mut *file)
        .take(LOG_FOLLOW_READ_CHUNK_BYTES as u64)
        .read_to_end(&mut state.pending)?;
    let bytes_read = state.pending.len().saturating_sub(pending_len);
    state.offset = state.offset.saturating_add(bytes_read as u64);
    if bytes_read == 0 {
        return Ok(Vec::new());
    }

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
    fn followed_paths_cache_discovery_until_reconciliation() {
        let mut paths = FollowedLogPaths::with_refresh_interval(
            vec![PathBuf::from("initial")],
            Duration::from_secs(60),
        );
        let mut discoveries = 0;

        assert_eq!(
            paths.refresh(|| {
                discoveries += 1;
                vec![PathBuf::from("unexpected")]
            }),
            [PathBuf::from("initial")]
        );
        assert_eq!(discoveries, 0);
    }

    #[test]
    fn followed_log_state_is_pruned_to_current_paths() {
        let mut followed = BTreeMap::from([
            (PathBuf::from("active"), FollowedLog::default()),
            (PathBuf::from("rotated"), FollowedLog::default()),
        ]);
        retain_followed_logs(&mut followed, &[PathBuf::from("active")]);
        assert_eq!(followed.len(), 1);
        assert!(followed.contains_key(Path::new("active")));
    }

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
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn followed_log_reopens_replaced_and_truncated_files_without_replaying_history() {
        let root = std::env::temp_dir().join(format!(
            "prodex-log-follow-rotation-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("runtime.log");
        fs::write(&path, "old\n").unwrap();
        let mut state = FollowedLog::default();
        assert_eq!(
            collect_new_followed_lines(&path, &mut state).unwrap(),
            ["old"]
        );

        fs::rename(&path, root.join("runtime.log.1")).unwrap();
        fs::write(&path, "replacement\n").unwrap();
        assert_eq!(
            collect_new_followed_lines(&path, &mut state).unwrap(),
            ["replacement"]
        );

        fs::write(&path, "x\n").unwrap();
        assert_eq!(
            collect_new_followed_lines(&path, &mut state).unwrap(),
            ["x"]
        );
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn followed_log_reads_one_append_after_large_history() {
        let root = std::env::temp_dir().join(format!(
            "prodex-log-follow-history-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("runtime.log");
        let history = (0..10_000)
            .map(|index| format!("history-{index}\n"))
            .collect::<String>();
        fs::write(&path, history).unwrap();
        let mut state = FollowedLog::at_end(&path);

        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"history-10000\n")
            .unwrap();
        assert_eq!(
            collect_new_followed_lines(&path, &mut state).unwrap(),
            ["history-10000"]
        );
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }
}
