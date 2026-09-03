use super::{PROCESS_ANCESTRY_LIMIT, PromptInjectionError, TARGET_ENV_KEYS};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TargetEnvironment {
    pub(crate) home: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) codex_sqlite_home: PathBuf,
    pub(crate) pwd: String,
}

impl TargetEnvironment {
    pub(crate) fn from_details(
        details: &ProcessDetails,
        workspace_root: &Path,
    ) -> std::result::Result<Self, PromptInjectionError> {
        let value = |key: &str| {
            details
                .environment
                .get(key)
                .filter(|value| !value.is_empty() && !value.as_bytes().contains(&0))
                .cloned()
                .ok_or(PromptInjectionError::TargetEnvironmentUnavailable)
        };
        let home = value("HOME")?;
        let codex_home = absolute_environment_path(&value("CODEX_HOME")?)?;
        let codex_sqlite_home = absolute_environment_path(&value("CODEX_SQLITE_HOME")?)?;
        let pwd = value("PWD")?;
        let pwd_path = Path::new(&pwd)
            .canonicalize()
            .map_err(|_| PromptInjectionError::TargetEnvironmentUnavailable)?;
        if pwd_path != workspace_root {
            return Err(PromptInjectionError::TargetEnvironmentUnavailable);
        }
        Ok(Self {
            home,
            codex_home,
            codex_sqlite_home,
            pwd,
        })
    }
}

fn absolute_environment_path(value: &str) -> std::result::Result<PathBuf, PromptInjectionError> {
    let path = PathBuf::from(value);
    path.is_absolute()
        .then_some(path)
        .ok_or(PromptInjectionError::TargetEnvironmentUnavailable)
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedTarget {
    pub(crate) prodex: ProcessRecord,
    pub(crate) writer: ProcessRecord,
    pub(crate) thread_id: String,
    pub(crate) queue_db: PathBuf,
    pub(crate) state_db: PathBuf,
    pub(crate) environment: TargetEnvironment,
    pub(crate) remote_endpoint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProcessRecord {
    pub(crate) pid: u32,
    pub(crate) parent_pid: u32,
    pub(crate) uid: u32,
    pub(crate) state: ProcessState,
    pub(crate) executable: PathBuf,
    pub(crate) argv: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) start_time: Option<u64>,
    pub(crate) birth_identity: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessState {
    Running,
    Stopped,
    Zombie,
    Dead,
}

impl ProcessState {
    pub(crate) const fn live(self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenProcessFile {
    pub(crate) path: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessDetails {
    pub(crate) record: ProcessRecord,
    pub(crate) environment: BTreeMap<String, String>,
    pub(crate) open_files: Vec<OpenProcessFile>,
}

pub(crate) trait ProcessInspector {
    fn current_uid(&self) -> std::result::Result<u32, PromptInjectionError>;
    fn list(&self) -> std::result::Result<Vec<ProcessRecord>, PromptInjectionError>;
    fn inspect(
        &self,
        pid: u32,
    ) -> std::result::Result<Option<ProcessDetails>, PromptInjectionError>;
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SystemProcessInspector;

#[cfg(target_os = "linux")]
impl ProcessInspector for SystemProcessInspector {
    fn current_uid(&self) -> std::result::Result<u32, PromptInjectionError> {
        // SAFETY: geteuid has no preconditions and only reads process credentials.
        Ok(unsafe { libc::geteuid() })
    }

    fn list(&self) -> std::result::Result<Vec<ProcessRecord>, PromptInjectionError> {
        let entries =
            fs::read_dir("/proc").map_err(|_| PromptInjectionError::VerificationInconclusive)?;
        Ok(entries
            .flatten()
            .filter_map(|entry| {
                let pid = entry.file_name().to_string_lossy().parse().ok()?;
                read_linux_process_record(pid).ok()
            })
            .collect())
    }

    fn inspect(
        &self,
        pid: u32,
    ) -> std::result::Result<Option<ProcessDetails>, PromptInjectionError> {
        let Some(record) = read_linux_process_record(pid).ok() else {
            return Ok(None);
        };
        let environment = read_linux_target_environment(pid)?;
        let open_files = read_linux_authoritative_open_files(pid)?;
        Ok(Some(ProcessDetails {
            record,
            environment,
            open_files,
        }))
    }
}

#[cfg(not(target_os = "linux"))]
impl ProcessInspector for SystemProcessInspector {
    fn current_uid(&self) -> std::result::Result<u32, PromptInjectionError> {
        Err(PromptInjectionError::VerificationInconclusive)
    }

    fn list(&self) -> std::result::Result<Vec<ProcessRecord>, PromptInjectionError> {
        Err(PromptInjectionError::VerificationInconclusive)
    }

    fn inspect(
        &self,
        _pid: u32,
    ) -> std::result::Result<Option<ProcessDetails>, PromptInjectionError> {
        Err(PromptInjectionError::VerificationInconclusive)
    }
}

#[cfg(target_os = "linux")]
fn read_linux_process_record(pid: u32) -> std::io::Result<ProcessRecord> {
    let root = PathBuf::from("/proc").join(pid.to_string());
    let status = fs::read_to_string(root.join("status"))?;
    let uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:")?.split_whitespace().next())
        .ok_or_else(|| std::io::Error::other("process uid unavailable"))?
        .parse::<u32>()
        .map_err(|_| std::io::Error::other("process uid invalid"))?;
    let state = status
        .lines()
        .find_map(|line| line.strip_prefix("State:")?.split_whitespace().next())
        .and_then(|value| value.chars().next())
        .map_or(ProcessState::Dead, linux_process_state);
    let stat = fs::read_to_string(root.join("stat"))?;
    let end_of_comm = stat
        .rfind(')')
        .ok_or_else(|| std::io::Error::other("process stat malformed"))?;
    let stat_fields = stat[end_of_comm + 1..]
        .split_whitespace()
        .collect::<Vec<_>>();
    let parent_pid = stat_fields
        .get(1)
        .ok_or_else(|| std::io::Error::other("process parent unavailable"))?
        .parse::<u32>()
        .map_err(|_| std::io::Error::other("process parent invalid"))?;
    let start_time = stat_fields.get(19).and_then(|value| value.parse().ok());
    let argv = fs::read(root.join("cmdline"))?
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
        .map(|value| String::from_utf8(value.to_vec()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| std::io::Error::other("process argv invalid"))?;
    if argv.is_empty() {
        return Err(std::io::Error::other("process argv unavailable"));
    }
    let executable = fs::canonicalize(root.join("exe"))?;
    let cwd = fs::canonicalize(root.join("cwd"))?;
    Ok(ProcessRecord {
        pid,
        parent_pid,
        uid,
        state,
        executable,
        argv,
        cwd,
        start_time,
        birth_identity: crate::runtime_process_birth_identity(pid),
    })
}

#[cfg(target_os = "linux")]
fn linux_process_state(value: char) -> ProcessState {
    match value {
        'R' | 'S' | 'D' | 'I' => ProcessState::Running,
        'T' | 't' => ProcessState::Stopped,
        'Z' => ProcessState::Zombie,
        _ => ProcessState::Dead,
    }
}

#[cfg(target_os = "linux")]
fn read_linux_target_environment(
    pid: u32,
) -> std::result::Result<BTreeMap<String, String>, PromptInjectionError> {
    let raw = fs::read(PathBuf::from("/proc").join(pid.to_string()).join("environ"))
        .map_err(|_| PromptInjectionError::TargetEnvironmentUnavailable)?;
    let mut environment = BTreeMap::new();
    for entry in raw.split(|byte| *byte == 0) {
        let Some(separator) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let (key, value) = entry.split_at(separator);
        let value = &value[1..];
        let Ok(key) = std::str::from_utf8(key) else {
            continue;
        };
        if !TARGET_ENV_KEYS.contains(&key) {
            continue;
        }
        let value = std::str::from_utf8(value)
            .map_err(|_| PromptInjectionError::TargetEnvironmentUnavailable)?;
        environment.insert(key.to_string(), value.to_string());
    }
    Ok(environment)
}

#[cfg(target_os = "linux")]
fn read_linux_authoritative_open_files(
    pid: u32,
) -> std::result::Result<Vec<OpenProcessFile>, PromptInjectionError> {
    let directory = fs::read_dir(PathBuf::from("/proc").join(pid.to_string()).join("fd"))
        .map_err(|_| PromptInjectionError::ThreadIdentityUnavailable)?;
    let unix_sockets = read_linux_unix_socket_paths();
    Ok(directory
        .flatten()
        .filter_map(|entry| {
            let path = fs::read_link(entry.path()).ok()?;
            if let Some(inode) = path
                .to_string_lossy()
                .strip_prefix("socket:[")
                .and_then(|value| value.strip_suffix(']'))
            {
                let socket_path = unix_sockets.get(inode)?.clone();
                return is_control_socket(&socket_path)
                    .then_some(OpenProcessFile { path: socket_path });
            }
            is_authoritative_file(&path).then_some(OpenProcessFile { path })
        })
        .collect())
}

#[cfg(target_os = "linux")]
fn read_linux_unix_socket_paths() -> BTreeMap<String, PathBuf> {
    let Ok(content) = fs::read_to_string("/proc/net/unix") else {
        return BTreeMap::new();
    };
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let inode = fields.get(6)?.to_string();
            let path = PathBuf::from(*fields.get(7)?);
            Some((inode, path))
        })
        .collect()
}

fn is_authoritative_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name == "queue_1.sqlite"
        || (name.starts_with("state_") && name.ends_with(".sqlite"))
        || (name.ends_with(".lock")
            && path
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                == Some("thread-writer-locks"))
        || is_rollout_file_name(name)
}

pub(crate) fn is_control_socket(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name == ".s" || name == ".prodex-session.sock")
        || (path.extension().and_then(|value| value.to_str()) == Some("sock")
            && path
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                == Some("app-server-control"))
}

pub(crate) fn is_rollout_file_name(name: &str) -> bool {
    name.starts_with("rollout-") && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
}

pub(crate) fn is_plain_prodex_session(process: &ProcessRecord) -> bool {
    executable_name(process).is_some_and(|name| name.eq_ignore_ascii_case("prodex"))
        && process.argv.get(1).is_some_and(|arg| arg == "s")
        && !process.argv.iter().skip(2).any(|arg| {
            matches!(arg.as_str(), "expose" | "super" | "exec" | "review")
                || arg.starts_with("prodex_super_")
                || arg.contains("__sub-agent")
                || arg.contains("release-smoke")
                || arg.contains("release_smoke")
        })
}

pub(crate) fn is_codex_writer(process: &ProcessRecord) -> bool {
    if !executable_name(process).is_some_and(|name| name.eq_ignore_ascii_case("codex")) {
        return false;
    }
    let command = first_codex_positional_arg(&process.argv);
    if has_remote_argument(&process.argv) {
        return false;
    }
    command.is_none_or(|command| matches!(command, "resume" | "app-server"))
}

fn executable_name(process: &ProcessRecord) -> Option<&str> {
    process
        .executable
        .file_name()
        .and_then(|value| value.to_str())
}

pub(crate) fn first_codex_positional_arg(argv: &[String]) -> Option<&str> {
    let mut index = 1;
    while index < argv.len() {
        let argument = argv[index].as_str();
        if argument == "--" {
            return None;
        }
        if argument.starts_with('-') {
            index += usize::from(codex_option_takes_value(argument));
        } else {
            return Some(argument);
        }
        index += 1;
    }
    None
}

fn codex_option_takes_value(argument: &str) -> bool {
    matches!(
        argument,
        "-c" | "--config"
            | "-i"
            | "--image"
            | "-m"
            | "--model"
            | "-p"
            | "--profile"
            | "-s"
            | "--sandbox"
            | "-C"
            | "--cd"
            | "--add-dir"
            | "-a"
            | "--ask-for-approval"
            | "--remote"
            | "--remote-auth-token-env"
            | "--thread-source"
    )
}

fn has_remote_argument(argv: &[String]) -> bool {
    argv.iter()
        .any(|argument| argument == "--remote" || argument.starts_with("--remote="))
}

pub(crate) fn is_descendant_of(
    pid: u32,
    ancestor: u32,
    by_pid: &HashMap<u32, &ProcessRecord>,
) -> bool {
    let mut current = pid;
    for _ in 0..PROCESS_ANCESTRY_LIMIT {
        let Some(process) = by_pid.get(&current) else {
            return false;
        };
        if !process.state.live() || process.birth_identity.is_none() {
            return false;
        }
        if process.parent_pid == ancestor {
            return by_pid.get(&ancestor).is_some_and(|parent| {
                parent.state.live() && parent.birth_identity.is_some() && parent.uid == process.uid
            });
        }
        if process.parent_pid == current {
            return false;
        }
        if by_pid
            .get(&process.parent_pid)
            .is_none_or(|parent| parent.uid != process.uid)
        {
            return false;
        }
        current = process.parent_pid;
    }
    false
}

pub(crate) fn same_process_identity(left: &ProcessRecord, right: &ProcessRecord) -> bool {
    left.pid == right.pid
        && left.parent_pid == right.parent_pid
        && left.uid == right.uid
        && left.executable == right.executable
        && left.argv == right.argv
        && left.cwd == right.cwd
        && left.start_time == right.start_time
        && left.birth_identity == right.birth_identity
}
