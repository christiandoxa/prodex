use anyhow::{Context, Result};
use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
#[cfg(unix)]
use std::sync::atomic::{AtomicUsize, Ordering};

const COMMAND_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_PROBE_OUTPUT_MAX_BYTES: usize = 1024 * 1024;

#[cfg(unix)]
static INTERACTIVE_SIGINT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(unix)]
extern "C" fn interactive_sigint_handler(_signal: libc::c_int) {
    INTERACTIVE_SIGINT_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[cfg(unix)]
pub(crate) struct InteractiveSigintGuard {
    previous: libc::sigaction,
}

#[cfg(unix)]
impl InteractiveSigintGuard {
    pub(crate) fn install() -> io::Result<Self> {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
        let empty_mask_result = unsafe { libc::sigemptyset(&mut action.sa_mask) };
        if empty_mask_result != 0 {
            return Err(io::Error::last_os_error());
        }
        action.sa_sigaction = interactive_sigint_handler as *const () as usize;
        if unsafe { libc::sigaction(libc::SIGINT, &action, &mut previous) } != 0 {
            return Err(io::Error::last_os_error());
        }
        INTERACTIVE_SIGINT_COUNT.store(0, Ordering::Relaxed);
        Ok(Self { previous })
    }

    pub(crate) fn count() -> usize {
        INTERACTIVE_SIGINT_COUNT.load(Ordering::Relaxed)
    }
}

#[cfg(unix)]
impl Drop for InteractiveSigintGuard {
    fn drop(&mut self) {
        let _ = unsafe { libc::sigaction(libc::SIGINT, &self.previous, std::ptr::null_mut()) };
        INTERACTIVE_SIGINT_COUNT.store(0, Ordering::Relaxed);
    }
}

#[cfg(unix)]
pub(crate) fn reset_child_sigint_handler(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            let mut action: libc::sigaction = std::mem::zeroed();
            if libc::sigemptyset(&mut action.sa_mask) != 0 {
                return Err(io::Error::last_os_error());
            }
            action.sa_sigaction = libc::SIG_DFL;
            if libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

pub(crate) fn configure_child_process_group(_command: &mut Command, _private_process_group: bool) {
    #[cfg(unix)]
    if _private_process_group {
        use std::os::unix::process::CommandExt;
        _command.process_group(0);
    }
}

/// Arrange for a directly owned child to receive a termination signal when this
/// process dies unexpectedly on Linux. Other platforms retain their existing
/// process-tree cleanup behavior.
pub(crate) fn configure_child_parent_death(command: &mut Command) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let parent_pid = unsafe { libc::getpid() };
        // SAFETY: the closure uses only async-signal-safe libc calls before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid() != parent_pid {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "parent exited before child startup",
                    ));
                }
                Ok(())
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}

pub(crate) fn terminate_child_process_group_best_effort(
    _child: &Child,
    _private_process_group: bool,
) -> bool {
    #[cfg(unix)]
    if _private_process_group {
        return signal_child_process_group(_child, libc::SIGKILL).is_ok();
    }
    false
}

#[cfg(unix)]
fn signal_child_process_group(child: &Child, signal: libc::c_int) -> io::Result<()> {
    let pid = child.id() as libc::pid_t;
    if pid <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "child process id must be positive",
        ));
    }
    if unsafe { libc::kill(-pid, signal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
pub(crate) fn terminate_child_gracefully(
    child: &mut Child,
    private_process_group: bool,
) -> io::Result<()> {
    if (private_process_group && signal_child_process_group(child, libc::SIGTERM).is_ok())
        || child.try_wait()?.is_some()
    {
        Ok(())
    } else {
        child.kill()
    }
}

pub(crate) fn terminate_child_process_tree(
    child: &mut Child,
    _private_process_group: bool,
) -> io::Result<()> {
    #[cfg(unix)]
    {
        if (_private_process_group && signal_child_process_group(child, libc::SIGKILL).is_ok())
            || child.try_wait()?.is_some()
        {
            return Ok(());
        }
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .env_remove("CONTROL_PLANE_API_KEY")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() || child.try_wait()?.is_some() {
            return Ok(());
        }
    }
    child.kill()
}

pub(crate) fn command_output_with_timeout(
    command: &mut Command,
    timeout: Duration,
    max_output_bytes: usize,
    label: &str,
) -> Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_process_group(command, true);
    configure_child_parent_death(command);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start {label}"))?;
    #[cfg(windows)]
    let child_job = assign_command_output_job(&child).ok();
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;
    let stdout_reader = bounded_output_reader(stdout, max_output_bytes);
    let stderr_reader = bounded_output_reader(stderr, max_output_bytes);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = terminate_child_process_tree(&mut child, true);
                let _ = child.wait();
                #[cfg(windows)]
                drop(child_job);
                anyhow::bail!("{label} timed out");
            }
            Err(error) => {
                let _ = terminate_child_process_tree(&mut child, true);
                let _ = child.wait();
                #[cfg(windows)]
                drop(child_job);
                return Err(error).with_context(|| format!("failed to poll {label}"));
            }
        }
    };
    #[cfg(windows)]
    drop(child_job);
    let stdout = join_bounded_output_reader(stdout_reader, &mut child, label)?;
    let stderr = join_bounded_output_reader(stderr_reader, &mut child, label)?;
    if stdout.len() > max_output_bytes || stderr.len() > max_output_bytes {
        anyhow::bail!("{label} output exceeded the limit");
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(windows)]
fn assign_command_output_job(child: &Child) -> io::Result<OwnedHandle> {
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw_job.is_null() {
        return Err(io::Error::last_os_error());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } != 0;
    if !configured {
        return Err(io::Error::last_os_error());
    }
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

pub(crate) fn command_probe_output(command: &mut Command, label: &str) -> Result<Output> {
    command_output_with_timeout(
        command,
        COMMAND_PROBE_TIMEOUT,
        COMMAND_PROBE_OUTPUT_MAX_BYTES,
        label,
    )
}

pub(crate) fn join_thread_with_timeout<T>(
    thread: thread::JoinHandle<T>,
    timeout: Duration,
    label: &str,
) -> Result<T> {
    let deadline = Instant::now() + timeout;
    while !thread.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !thread.is_finished() {
        anyhow::bail!("{label} did not stop");
    }
    thread
        .join()
        .map_err(|_| anyhow::anyhow!("{label} thread panicked"))
}

fn bounded_output_reader(
    mut reader: impl Read + Send + 'static,
    max_output_bytes: usize,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(max_output_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_bounded_output_reader(
    reader: thread::JoinHandle<io::Result<Vec<u8>>>,
    child: &mut Child,
    label: &str,
) -> Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_millis(250);
    while !reader.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !reader.is_finished() {
        let _ = terminate_child_process_tree(child, true);
        let deadline = Instant::now() + Duration::from_millis(250);
        while !reader.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    if !reader.is_finished() {
        anyhow::bail!("{label} output did not close");
    }
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("{label} output reader panicked"))?
        .with_context(|| format!("failed to read {label} output"))
}

#[cfg(windows)]
pub(crate) fn terminate_child_gracefully(
    child: &mut Child,
    _private_process_group: bool,
) -> io::Result<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() || child.try_wait()?.is_some() {
        Ok(())
    } else {
        child.kill()
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn terminate_child_gracefully(
    child: &mut Child,
    _private_process_group: bool,
) -> io::Result<()> {
    child.kill()
}
