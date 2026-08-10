use super::{
    ChildLaunchSpec, SUB_AGENT_CHILD_REAP_TIMEOUT, SUB_AGENT_LAUNCHER_MARKER,
    SUB_AGENT_OUTPUT_DRAIN_TIMEOUT, SUB_AGENT_RECURSION_MARKER, child_argv,
};
use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::process::Stdio;

pub(super) struct SubAgentChildOutcome {
    pub(super) status: std::process::ExitStatus,
    pub(super) cancelled: bool,
    pub(super) output_incomplete: bool,
    pub(super) output_bytes: u64,
}

pub(super) async fn run_child(
    spec: &ChildLaunchSpec,
    task: &str,
    task_path: &Path,
) -> Result<SubAgentChildOutcome> {
    let mut command = tokio::process::Command::new(&spec.executable);
    command
        .args(child_argv(spec, task))
        .env(SUB_AGENT_RECURSION_MARKER, "1")
        .env_remove(SUB_AGENT_LAUNCHER_MARKER)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_sub_agent_child_process_group(&mut command);
    let mut child = command.spawn().context("failed to spawn sub-agent child")?;
    let process_group_id = child.id();
    #[cfg(windows)]
    let mut child_job = match assign_sub_agent_child_job(&child) {
        Ok(job) => Some(job),
        Err(error) => {
            let _ = terminate_sub_agent_child(&mut child, process_group_id);
            let _ = child.wait().await;
            return Err(error).context("failed to contain sub-agent process tree");
        }
    };
    if let Err(error) = fs::remove_file(task_path) {
        #[cfg(windows)]
        drop(child_job.take());
        let _ = terminate_sub_agent_child(&mut child, process_group_id);
        let _ = child.wait().await;
        return Err(error).context("failed to remove consumed task file");
    }
    let stdout = child.stdout.take().context("child stdout pipe missing")?;
    let stderr = child.stderr.take().context("child stderr pipe missing")?;
    let stdout_task = tokio::spawn(relay_child_output(stdout, tokio::io::stdout()));
    let stderr_task = tokio::spawn(relay_child_output(stderr, tokio::io::stderr()));
    let mut cancelled = false;
    let status = tokio::select! {
        status = child.wait() => status.context("failed to wait for sub-agent child")?,
        signal = sub_agent_shutdown_signal() => {
            signal?;
            cancelled = true;
            #[cfg(windows)]
            drop(child_job.take());
            terminate_sub_agent_child(&mut child, process_group_id)
                .context("failed to terminate cancelled sub-agent child")?;
            tokio::time::timeout(SUB_AGENT_CHILD_REAP_TIMEOUT, child.wait())
                .await
                .context("timed out while reaping cancelled sub-agent child")?
                .context("failed to reap cancelled sub-agent child")?
        }
    };
    #[cfg(windows)]
    drop(child_job.take());
    terminate_sub_agent_process_group_best_effort(process_group_id);
    let output = drain_child_output_tasks(stdout_task, stderr_task).await;
    let output_incomplete = output.is_err();
    let output_bytes = output.unwrap_or_default();
    Ok(SubAgentChildOutcome {
        status,
        cancelled,
        output_incomplete,
        output_bytes,
    })
}

pub(super) fn configure_sub_agent_child_process_group(_command: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        _command.as_std_mut().process_group(0);
    }
}

#[cfg(windows)]
pub(super) fn assign_sub_agent_child_job(child: &tokio::process::Child) -> io::Result<OwnedHandle> {
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
    let child_handle = child.raw_handle().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "sub-agent child exited before job assignment",
        )
    })?;
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child_handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

pub(super) fn terminate_sub_agent_child(
    child: &mut tokio::process::Child,
    process_group_id: Option<u32>,
) -> io::Result<()> {
    if terminate_sub_agent_process_group_best_effort(process_group_id)
        || child.try_wait()?.is_some()
    {
        Ok(())
    } else {
        child.start_kill()
    }
}

pub(super) fn terminate_sub_agent_process_group_best_effort(
    _process_group_id: Option<u32>,
) -> bool {
    #[cfg(unix)]
    if let Some(process_group_id) = _process_group_id
        && let Ok(process_group_id) = libc::pid_t::try_from(process_group_id)
        && process_group_id > 0
    {
        return unsafe { libc::kill(-process_group_id, libc::SIGKILL) } == 0;
    }
    #[cfg(windows)]
    if let Some(process_group_id) = _process_group_id {
        return std::process::Command::new("taskkill")
            .args(["/PID", &process_group_id.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }
    false
}

pub(super) async fn drain_child_output_tasks(
    mut stdout_task: tokio::task::JoinHandle<io::Result<u64>>,
    mut stderr_task: tokio::task::JoinHandle<io::Result<u64>>,
) -> Result<u64> {
    let drained = tokio::time::timeout(SUB_AGENT_OUTPUT_DRAIN_TIMEOUT, async {
        let (stdout, stderr) = tokio::join!(&mut stdout_task, &mut stderr_task);
        let stdout = stdout.context("sub-agent stdout relay task failed")??;
        let stderr = stderr.context("sub-agent stderr relay task failed")??;
        Ok::<_, anyhow::Error>(stdout.saturating_add(stderr))
    })
    .await;
    match drained {
        Ok(result) => result,
        Err(_) => {
            stdout_task.abort();
            stderr_task.abort();
            if !stdout_task.is_finished() {
                let _ = stdout_task.await;
            }
            if !stderr_task.is_finished() {
                let _ = stderr_task.await;
            }
            bail!("sub-agent output drain timed out after child exit");
        }
    }
}

pub(super) async fn relay_child_output<R, W>(mut reader: R, mut writer: W) -> io::Result<u64>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buffer = [0_u8; 8_192];
    let mut write_error = None;
    let mut bytes_read = 0_u64;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        bytes_read = bytes_read.saturating_add(read as u64);
        if write_error.is_none()
            && let Err(error) = writer.write_all(&buffer[..read]).await
        {
            write_error = Some(error);
        }
    }
    if let Some(error) = write_error {
        Err(error)
    } else {
        writer.flush().await.map(|()| bytes_read)
    }
}

async fn sub_agent_shutdown_signal() -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}
