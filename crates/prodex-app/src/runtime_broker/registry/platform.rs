#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStringExt as _;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::time::Instant;

#[cfg(target_os = "macos")]
use super::RuntimeProcessIdentityOutcome;
use super::RuntimeProcessTerminationOutcome;
#[cfg(not(target_os = "macos"))]
use crate::{ProcessRow, collect_process_rows};

pub(super) trait RuntimeProcessPlatform {
    fn pid_alive(pid: u32) -> bool;
    fn process_absence_proven(pid: u32) -> bool;
    fn process_birth_identity(pid: u32) -> Option<String>;
    fn executable_path(pid: u32) -> Option<PathBuf>;
    fn terminate(
        pid: u32,
        expected_birth_identity: Option<&str>,
        expected_executable_path: Option<&Path>,
    ) -> RuntimeProcessTerminationOutcome;
}

#[cfg(target_os = "linux")]
pub(super) struct RuntimeProcessLinux;

#[cfg(windows)]
pub(super) struct RuntimeProcessWindows;

#[cfg(target_os = "macos")]
pub(super) struct RuntimeProcessMacos;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) struct RuntimeProcessFallback;

pub(super) type RuntimeProcessPlatformImpl = RuntimeProcessPlatformForTarget;

#[cfg(target_os = "linux")]
pub(super) type RuntimeProcessPlatformForTarget = RuntimeProcessLinux;

#[cfg(windows)]
pub(super) type RuntimeProcessPlatformForTarget = RuntimeProcessWindows;

#[cfg(target_os = "macos")]
pub(super) type RuntimeProcessPlatformForTarget = RuntimeProcessMacos;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) type RuntimeProcessPlatformForTarget = RuntimeProcessFallback;

#[cfg(not(target_os = "macos"))]
pub(super) fn runtime_process_row(pid: u32) -> Option<ProcessRow> {
    collect_process_rows()
        .into_iter()
        .find(|row| row.pid == pid)
}

#[cfg(target_os = "linux")]
impl RuntimeProcessPlatform for RuntimeProcessLinux {
    fn pid_alive(pid: u32) -> bool {
        PathBuf::from(format!("/proc/{pid}")).exists()
    }

    fn process_absence_proven(pid: u32) -> bool {
        if linux_process_is_zombie(pid) {
            return true;
        }
        match linux_pidfd_open(pid) {
            Ok(_) => false,
            Err(error) => error.raw_os_error() == Some(libc::ESRCH),
        }
    }

    fn process_birth_identity(pid: u32) -> Option<String> {
        let _pidfd = linux_pidfd_open(pid).ok()?;
        linux_process_birth_identity(pid)
    }

    fn executable_path(pid: u32) -> Option<PathBuf> {
        fs::read_link(format!("/proc/{pid}/exe")).ok()
    }

    fn terminate(
        pid: u32,
        expected_birth_identity: Option<&str>,
        _expected_executable_path: Option<&Path>,
    ) -> RuntimeProcessTerminationOutcome {
        terminate_runtime_process_linux(pid, expected_birth_identity)
    }
}

#[cfg(target_os = "linux")]
fn linux_pidfd_open(pid: u32) -> io::Result<OwnedFd> {
    let pid = libc::pid_t::try_from(pid).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "pid does not fit the platform pid_t",
        )
    })?;
    if pid <= 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid pid"));
    }
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0 as libc::c_uint) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = i32::try_from(fd).map_err(|_| io::Error::other("pidfd did not fit RawFd"))?;
    // SAFETY: successful pidfd_open returns an owned file descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(target_os = "linux")]
fn linux_process_birth_identity(pid: u32) -> Option<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let start_time = stat.rsplit_once(") ")?.1.split_whitespace().nth(19)?;
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    let boot_id = boot_id.trim();
    (!boot_id.is_empty() && !start_time.is_empty()).then(|| format!("linux:{boot_id}:{start_time}"))
}

#[cfg(target_os = "linux")]
fn linux_process_is_zombie(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| stat.rsplit_once(") ").map(|(_, rest)| rest.to_string()))
        .and_then(|rest| rest.split_whitespace().next().map(str::to_string))
        .is_some_and(|state| state == "Z")
}

#[cfg(target_os = "linux")]
fn linux_pidfd_send_signal(pidfd: &OwnedFd, signal: libc::c_int) -> io::Result<()> {
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0 as libc::c_uint,
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum LinuxPidfdWait {
    Exited,
    TimedOut,
    Failed,
}

#[cfg(target_os = "linux")]
fn linux_pidfd_wait(pidfd: &OwnedFd, timeout: Duration) -> LinuxPidfdWait {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout_ms = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
        let mut pollfd = libc::pollfd {
            fd: pidfd.as_raw_fd(),
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if result > 0 {
            return if pollfd.revents != 0 {
                LinuxPidfdWait::Exited
            } else {
                LinuxPidfdWait::Failed
            };
        }
        if result == 0 {
            return LinuxPidfdWait::TimedOut;
        }
        if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return LinuxPidfdWait::Failed;
        }
        if deadline <= Instant::now() {
            return LinuxPidfdWait::TimedOut;
        }
    }
}

#[cfg(target_os = "linux")]
fn terminate_runtime_process_linux(
    pid: u32,
    expected_birth_identity: Option<&str>,
) -> RuntimeProcessTerminationOutcome {
    let Some(expected_birth_identity) = expected_birth_identity else {
        return RuntimeProcessTerminationOutcome::OwnershipUnproven;
    };
    let pidfd = match linux_pidfd_open(pid) {
        Ok(pidfd) => pidfd,
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {
            return RuntimeProcessTerminationOutcome::NotRunning;
        }
        Err(_) => return RuntimeProcessTerminationOutcome::OwnershipUnproven,
    };

    match linux_process_birth_identity(pid) {
        Some(actual) if actual == expected_birth_identity => {}
        Some(_) => return RuntimeProcessTerminationOutcome::OwnershipChanged,
        None => {
            return match linux_pidfd_wait(&pidfd, Duration::ZERO) {
                LinuxPidfdWait::Exited => RuntimeProcessTerminationOutcome::NotRunning,
                LinuxPidfdWait::TimedOut | LinuxPidfdWait::Failed => {
                    RuntimeProcessTerminationOutcome::OwnershipUnproven
                }
            };
        }
    }

    match linux_pidfd_wait(&pidfd, Duration::ZERO) {
        LinuxPidfdWait::Exited => return RuntimeProcessTerminationOutcome::NotRunning,
        LinuxPidfdWait::Failed => return RuntimeProcessTerminationOutcome::OwnershipUnproven,
        LinuxPidfdWait::TimedOut => {}
    }

    if let Err(error) = linux_pidfd_send_signal(&pidfd, libc::SIGTERM) {
        return if error.raw_os_error() == Some(libc::ESRCH) {
            RuntimeProcessTerminationOutcome::Terminated
        } else {
            RuntimeProcessTerminationOutcome::OwnershipUnproven
        };
    }
    match linux_pidfd_wait(&pidfd, Duration::from_millis(500)) {
        LinuxPidfdWait::Exited => return RuntimeProcessTerminationOutcome::Terminated,
        LinuxPidfdWait::Failed => return RuntimeProcessTerminationOutcome::OwnershipUnproven,
        LinuxPidfdWait::TimedOut => {}
    }

    if let Err(error) = linux_pidfd_send_signal(&pidfd, libc::SIGKILL) {
        return if error.raw_os_error() == Some(libc::ESRCH) {
            RuntimeProcessTerminationOutcome::Terminated
        } else {
            RuntimeProcessTerminationOutcome::OwnershipUnproven
        };
    }
    match linux_pidfd_wait(&pidfd, Duration::from_millis(250)) {
        LinuxPidfdWait::Exited => RuntimeProcessTerminationOutcome::Terminated,
        LinuxPidfdWait::TimedOut => RuntimeProcessTerminationOutcome::StillRunning,
        LinuxPidfdWait::Failed => RuntimeProcessTerminationOutcome::OwnershipUnproven,
    }
}

#[cfg(windows)]
impl RuntimeProcessPlatform for RuntimeProcessWindows {
    fn pid_alive(pid: u32) -> bool {
        windows_open_process(pid).is_ok()
    }

    fn process_absence_proven(pid: u32) -> bool {
        windows_open_process(pid)
            .err()
            .and_then(|error| error.raw_os_error())
            .is_some_and(|error| {
                error == windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER as i32
            })
    }

    fn process_birth_identity(pid: u32) -> Option<String> {
        windows_open_process(pid)
            .ok()
            .and_then(|handle| windows_process_birth_identity(&handle))
    }

    fn executable_path(pid: u32) -> Option<PathBuf> {
        windows_open_process(pid)
            .ok()
            .and_then(|handle| windows_process_image_path(&handle))
    }

    fn terminate(
        pid: u32,
        expected_birth_identity: Option<&str>,
        _expected_executable_path: Option<&Path>,
    ) -> RuntimeProcessTerminationOutcome {
        terminate_runtime_process_windows(pid, expected_birth_identity)
    }
}

#[cfg(windows)]
fn windows_open_process(pid: u32) -> io::Result<OwnedHandle> {
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };

    let access = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE;
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: successful OpenProcess returns an owned process handle.
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}

#[cfg(windows)]
fn windows_process_birth_identity(handle: &OwnedHandle) -> Option<String> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;

    let mut creation_time = FILETIME::default();
    let mut exit_time = FILETIME::default();
    let mut kernel_time = FILETIME::default();
    let mut user_time = FILETIME::default();
    let success = unsafe {
        GetProcessTimes(
            handle.as_raw_handle(),
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        )
    } != 0;
    success.then(|| {
        let ticks = (u64::from(creation_time.dwHighDateTime) << 32)
            | u64::from(creation_time.dwLowDateTime);
        format!("windows:{ticks}")
    })
}

#[cfg(windows)]
fn windows_process_image_path(handle: &OwnedHandle) -> Option<PathBuf> {
    use windows_sys::Win32::System::Threading::{PROCESS_NAME_WIN32, QueryFullProcessImageNameW};

    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let success = unsafe {
        QueryFullProcessImageNameW(
            handle.as_raw_handle(),
            PROCESS_NAME_WIN32,
            buffer.as_mut_ptr(),
            &mut length,
        )
    } != 0;
    success.then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize])))
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum WindowsProcessWait {
    Exited,
    TimedOut,
    Failed,
}

#[cfg(windows)]
fn windows_process_wait(handle: &OwnedHandle, timeout: Duration) -> WindowsProcessWait {
    use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
    match unsafe { WaitForSingleObject(handle.as_raw_handle(), timeout_ms) } {
        WAIT_OBJECT_0 => WindowsProcessWait::Exited,
        WAIT_TIMEOUT => WindowsProcessWait::TimedOut,
        _ => WindowsProcessWait::Failed,
    }
}

#[cfg(windows)]
fn terminate_runtime_process_windows(
    pid: u32,
    expected_birth_identity: Option<&str>,
) -> RuntimeProcessTerminationOutcome {
    use windows_sys::Win32::System::Threading::TerminateProcess;

    let Some(expected_birth_identity) = expected_birth_identity else {
        return RuntimeProcessTerminationOutcome::OwnershipUnproven;
    };
    let handle = match windows_open_process(pid) {
        Ok(handle) => handle,
        Err(error)
            if error.raw_os_error()
                == Some(windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER as i32) =>
        {
            return RuntimeProcessTerminationOutcome::NotRunning;
        }
        Err(_) => return RuntimeProcessTerminationOutcome::OwnershipUnproven,
    };
    match windows_process_birth_identity(&handle) {
        Some(actual) if actual == expected_birth_identity => {}
        Some(_) => return RuntimeProcessTerminationOutcome::OwnershipChanged,
        None => {
            return match windows_process_wait(&handle, Duration::ZERO) {
                WindowsProcessWait::Exited => RuntimeProcessTerminationOutcome::NotRunning,
                WindowsProcessWait::TimedOut | WindowsProcessWait::Failed => {
                    RuntimeProcessTerminationOutcome::OwnershipUnproven
                }
            };
        }
    }
    match windows_process_wait(&handle, Duration::ZERO) {
        WindowsProcessWait::Exited => return RuntimeProcessTerminationOutcome::NotRunning,
        WindowsProcessWait::Failed => return RuntimeProcessTerminationOutcome::OwnershipUnproven,
        WindowsProcessWait::TimedOut => {}
    }
    if unsafe { TerminateProcess(handle.as_raw_handle(), 1) } == 0 {
        return match windows_process_wait(&handle, Duration::ZERO) {
            WindowsProcessWait::Exited => RuntimeProcessTerminationOutcome::Terminated,
            WindowsProcessWait::TimedOut | WindowsProcessWait::Failed => {
                RuntimeProcessTerminationOutcome::OwnershipUnproven
            }
        };
    }
    match windows_process_wait(&handle, Duration::from_millis(500)) {
        WindowsProcessWait::Exited => RuntimeProcessTerminationOutcome::Terminated,
        WindowsProcessWait::TimedOut => RuntimeProcessTerminationOutcome::StillRunning,
        WindowsProcessWait::Failed => RuntimeProcessTerminationOutcome::OwnershipUnproven,
    }
}

#[cfg(target_os = "macos")]
impl RuntimeProcessPlatform for RuntimeProcessMacos {
    fn pid_alive(pid: u32) -> bool {
        macos_pid_exists(pid)
    }

    fn process_absence_proven(pid: u32) -> bool {
        macos_process_bsdinfo(pid).is_ok_and(|info| info.pbi_status == libc::SZOMB)
            || macos_pid_absent(pid)
    }

    fn process_birth_identity(pid: u32) -> Option<String> {
        let info = macos_process_bsdinfo(pid).ok()?;
        Some(format!(
            "macos:{}:{}",
            info.pbi_start_tvsec, info.pbi_start_tvusec
        ))
    }

    fn executable_path(pid: u32) -> Option<PathBuf> {
        macos_process_executable_path(pid).ok()
    }

    fn terminate(
        pid: u32,
        expected_birth_identity: Option<&str>,
        expected_executable_path: Option<&Path>,
    ) -> RuntimeProcessTerminationOutcome {
        terminate_runtime_process_macos(pid, expected_birth_identity, expected_executable_path)
    }
}

#[cfg(target_os = "macos")]
fn macos_pid(pid: u32) -> Option<libc::pid_t> {
    let pid = libc::pid_t::try_from(pid).ok()?;
    (pid > 0).then_some(pid)
}

#[cfg(target_os = "macos")]
fn macos_pid_exists(pid: u32) -> bool {
    let Some(pid) = macos_pid(pid) else {
        return false;
    };
    // SAFETY: signal 0 only probes existence for a validated positive PID.
    (unsafe { libc::kill(pid, 0) }) == 0
        || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(target_os = "macos")]
fn macos_pid_absent(pid: u32) -> bool {
    let Some(pid) = macos_pid(pid) else {
        return true;
    };
    // SAFETY: signal 0 only probes existence for a validated positive PID.
    (unsafe { libc::kill(pid, 0) }) != 0
        && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

#[cfg(target_os = "macos")]
fn macos_process_bsdinfo(pid: u32) -> io::Result<libc::proc_bsdinfo> {
    let pid =
        macos_pid(pid).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid pid"))?;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let expected = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: buffer points to enough writable memory for PROC_PIDTBSDINFO.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            expected,
        )
    };
    if read != expected {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: proc_pidinfo initialized the complete structure above.
    Ok(unsafe { info.assume_init() })
}

#[cfg(target_os = "macos")]
fn macos_process_executable_path(pid: u32) -> io::Result<PathBuf> {
    let pid =
        macos_pid(pid).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid pid"))?;
    let mut buffer = vec![0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: buffer is writable for the supplied size and PID is positive.
    let read = unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if read <= 0 {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(read as usize);
    if let Some(nul) = buffer.iter().position(|byte| *byte == 0) {
        buffer.truncate(nul);
    }
    if buffer.is_empty() {
        return Err(io::Error::other("process executable path is empty"));
    }
    Ok(PathBuf::from(std::ffi::OsString::from_vec(buffer)))
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum MacosProcessWait {
    ExitedOrChanged,
    TimedOut,
    Failed,
}

#[cfg(target_os = "macos")]
fn macos_process_wait(
    pid: u32,
    expected_birth_identity: &str,
    expected_executable_path: &Path,
    timeout: Duration,
) -> MacosProcessWait {
    let deadline = Instant::now() + timeout;
    loop {
        match super::runtime_process_identity_outcome_for::<RuntimeProcessMacos>(
            pid,
            Some(expected_birth_identity),
            Some(expected_executable_path),
        ) {
            RuntimeProcessIdentityOutcome::Absent
            | RuntimeProcessIdentityOutcome::OwnershipChanged => {
                return MacosProcessWait::ExitedOrChanged;
            }
            RuntimeProcessIdentityOutcome::OwnershipUnproven => {
                return MacosProcessWait::Failed;
            }
            RuntimeProcessIdentityOutcome::Proven => {}
        }
        if Instant::now() >= deadline {
            return MacosProcessWait::TimedOut;
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(10)),
        );
    }
}

#[cfg(target_os = "macos")]
fn macos_send_signal(pid: u32, signal: libc::c_int) -> io::Result<()> {
    let pid =
        macos_pid(pid).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid pid"))?;
    // SAFETY: PID is positive and signal is a fixed POSIX process signal.
    if unsafe { libc::kill(pid, signal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn terminate_runtime_process_macos(
    pid: u32,
    expected_birth_identity: Option<&str>,
    expected_executable_path: Option<&Path>,
) -> RuntimeProcessTerminationOutcome {
    let (Some(expected_birth_identity), Some(expected_executable_path)) =
        (expected_birth_identity, expected_executable_path)
    else {
        return RuntimeProcessTerminationOutcome::OwnershipUnproven;
    };
    match super::runtime_process_identity_outcome_for::<RuntimeProcessMacos>(
        pid,
        Some(expected_birth_identity),
        Some(expected_executable_path),
    ) {
        RuntimeProcessIdentityOutcome::Absent => {
            return RuntimeProcessTerminationOutcome::NotRunning;
        }
        RuntimeProcessIdentityOutcome::OwnershipChanged => {
            return RuntimeProcessTerminationOutcome::OwnershipChanged;
        }
        RuntimeProcessIdentityOutcome::OwnershipUnproven => {
            return RuntimeProcessTerminationOutcome::OwnershipUnproven;
        }
        RuntimeProcessIdentityOutcome::Proven => {}
    }

    if let Err(error) = macos_send_signal(pid, libc::SIGTERM) {
        return if error.raw_os_error() == Some(libc::ESRCH) {
            RuntimeProcessTerminationOutcome::Terminated
        } else {
            RuntimeProcessTerminationOutcome::OwnershipUnproven
        };
    }
    match macos_process_wait(
        pid,
        expected_birth_identity,
        expected_executable_path,
        Duration::from_millis(500),
    ) {
        MacosProcessWait::ExitedOrChanged => {
            return RuntimeProcessTerminationOutcome::Terminated;
        }
        MacosProcessWait::Failed => {
            return RuntimeProcessTerminationOutcome::OwnershipUnproven;
        }
        MacosProcessWait::TimedOut => {}
    }

    if let Err(error) = macos_send_signal(pid, libc::SIGKILL) {
        return if error.raw_os_error() == Some(libc::ESRCH) {
            RuntimeProcessTerminationOutcome::Terminated
        } else {
            RuntimeProcessTerminationOutcome::OwnershipUnproven
        };
    }
    match macos_process_wait(
        pid,
        expected_birth_identity,
        expected_executable_path,
        Duration::from_millis(250),
    ) {
        MacosProcessWait::ExitedOrChanged => RuntimeProcessTerminationOutcome::Terminated,
        MacosProcessWait::TimedOut => RuntimeProcessTerminationOutcome::StillRunning,
        MacosProcessWait::Failed => RuntimeProcessTerminationOutcome::OwnershipUnproven,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
impl RuntimeProcessPlatform for RuntimeProcessFallback {
    fn pid_alive(_pid: u32) -> bool {
        false
    }

    fn process_absence_proven(pid: u32) -> bool {
        #[cfg(unix)]
        {
            let Ok(pid) = libc::pid_t::try_from(pid) else {
                return false;
            };
            // SAFETY: signal 0 only probes whether this validated PID exists.
            if pid <= 0 || unsafe { libc::kill(pid, 0) } == 0 {
                return false;
            }
            return io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        }
        #[cfg(not(unix))]
        false
    }

    fn process_birth_identity(_pid: u32) -> Option<String> {
        None
    }

    fn executable_path(_pid: u32) -> Option<PathBuf> {
        None
    }

    fn terminate(
        _pid: u32,
        _expected_birth_identity: Option<&str>,
        _expected_executable_path: Option<&Path>,
    ) -> RuntimeProcessTerminationOutcome {
        RuntimeProcessTerminationOutcome::OwnershipUnproven
    }
}
