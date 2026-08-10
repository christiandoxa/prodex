use crate::McpJsonlBridgeArgs;
use anyhow::{Context, Result, bail};
use prodex_mcp_stdio::{McpMessageFraming, read_mcp_message, write_mcp_message};
use std::io::{self, BufReader, BufWriter, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MCP_BRIDGE_MONITOR_INTERVAL: Duration = Duration::from_millis(10);
const MCP_BRIDGE_OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const MCP_BRIDGE_STDERR_LIMIT: usize = 64 * 1024;
type McpStderrReader = JoinHandle<io::Result<(Vec<u8>, bool)>>;

pub(crate) fn handle_mcp_jsonl_bridge(args: McpJsonlBridgeArgs) -> Result<()> {
    let mut command = Command::new(&args.command);
    command
        .args(&args.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::configure_child_process_group(&mut command, true);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start MCP server {}", args.command.display()))?;

    let child_stdout = child
        .stdout
        .take()
        .context("failed to capture MCP server stdout")?;
    let child_stdin = child
        .stdin
        .take()
        .context("failed to capture MCP server stdin")?;
    let child_stderr = child
        .stderr
        .take()
        .context("failed to capture MCP server stderr")?;
    let framing = Arc::new(Mutex::new(McpMessageFraming::ContentLength));
    let output_framing = Arc::clone(&framing);

    let output = std::thread::spawn(move || -> Result<()> {
        let mut reader = BufReader::new(child_stdout);
        let mut stdout = io::stdout().lock();
        while let Some((message, _)) = read_mcp_message(&mut reader)? {
            let framing = *output_framing
                .lock()
                .map_err(|_| anyhow::anyhow!("MCP bridge framing lock poisoned"))?;
            write_mcp_message(&mut stdout, &message, framing)?;
        }
        Ok(())
    });

    let input = std::thread::spawn(move || -> Result<()> {
        let mut child_stdin = BufWriter::new(child_stdin);
        let stdin = io::stdin();
        let mut stdin = BufReader::new(stdin.lock());
        while let Some((message, input_framing)) = read_mcp_message(&mut stdin)? {
            *framing
                .lock()
                .map_err(|_| anyhow::anyhow!("MCP bridge framing lock poisoned"))? = input_framing;
            write_mcp_message(&mut child_stdin, &message, McpMessageFraming::JsonLine)?;
        }
        Ok(())
    });
    let stderr = std::thread::spawn(move || read_bounded_mcp_stderr(child_stderr));

    monitor_mcp_bridge(&mut child, output, input, stderr)
}

fn monitor_mcp_bridge(
    child: &mut Child,
    output: JoinHandle<Result<()>>,
    input: JoinHandle<Result<()>>,
    stderr: McpStderrReader,
) -> Result<()> {
    let mut output = Some(output);
    let mut input = Some(input);
    let mut stderr = Some(stderr);
    loop {
        if output.as_ref().is_some_and(JoinHandle::is_finished) {
            return handle_finished_mcp_output(child, &mut output, &mut input, &mut stderr);
        }

        if let Some(status) = poll_mcp_child(child)? {
            return handle_exited_mcp_child(child, status, &mut output, &mut input, &mut stderr);
        }

        if input.as_ref().is_some_and(JoinHandle::is_finished) {
            return handle_finished_mcp_input(child, &mut output, &mut input, &mut stderr);
        }

        std::thread::sleep(MCP_BRIDGE_MONITOR_INTERVAL);
    }
}

fn handle_finished_mcp_input(
    child: &mut Child,
    output: &mut Option<JoinHandle<Result<()>>>,
    input: &mut Option<JoinHandle<Result<()>>>,
    stderr: &mut Option<McpStderrReader>,
) -> Result<()> {
    let input_result = join_mcp_bridge_pump(
        input.take().expect("finished input pump should exist"),
        "input",
    );
    let child_status = child.try_wait().ok().flatten();
    stop_mcp_child(child);
    let output_result =
        join_mcp_bridge_pump(output.take().expect("output pump should exist"), "output");
    let stderr = join_mcp_stderr(stderr)?;
    output_result?;
    input_result.and_then(|()| {
        child_status.map_or(Ok(()), |status| mcp_child_status_result(status, stderr))
    })
}

fn handle_finished_mcp_output(
    child: &mut Child,
    output: &mut Option<JoinHandle<Result<()>>>,
    input: &mut Option<JoinHandle<Result<()>>>,
    stderr: &mut Option<McpStderrReader>,
) -> Result<()> {
    if let Err(err) = join_mcp_bridge_pump(
        output.take().expect("finished output pump should exist"),
        "output",
    ) {
        stop_mcp_child(child);
        return Err(err);
    }
    if let Some(status) = poll_mcp_child(child)? {
        join_finished_mcp_input(input)?;
        return mcp_child_status_result(status, join_mcp_stderr(stderr)?);
    }
    stop_mcp_child(child);
    bail!("MCP server closed stdout before exiting");
}

fn handle_exited_mcp_child(
    child: &mut Child,
    status: ExitStatus,
    output: &mut Option<JoinHandle<Result<()>>>,
    input: &mut Option<JoinHandle<Result<()>>>,
    stderr: &mut Option<McpStderrReader>,
) -> Result<()> {
    stop_mcp_child(child);
    let output = output.take().expect("output pump should exist");
    let deadline = Instant::now() + MCP_BRIDGE_OUTPUT_DRAIN_TIMEOUT;
    while !output.is_finished() && Instant::now() < deadline {
        std::thread::sleep(MCP_BRIDGE_MONITOR_INTERVAL);
    }
    if !output.is_finished() {
        bail!("MCP server stdout remained open after child exited");
    }
    join_mcp_bridge_pump(output, "output")?;
    join_finished_mcp_input(input)?;
    mcp_child_status_result(status, join_mcp_stderr(stderr)?)
}

fn join_finished_mcp_input(input: &mut Option<JoinHandle<Result<()>>>) -> Result<()> {
    if input.as_ref().is_some_and(JoinHandle::is_finished) {
        join_mcp_bridge_pump(
            input.take().expect("finished input pump should exist"),
            "input",
        )?;
    }
    Ok(())
}

fn poll_mcp_child(child: &mut Child) -> Result<Option<ExitStatus>> {
    let result = child.try_wait();
    if result.is_err() {
        stop_mcp_child(child);
    }
    result.context("failed to poll MCP server")
}

fn join_mcp_bridge_pump(handle: JoinHandle<Result<()>>, name: &str) -> Result<()> {
    crate::join_thread_with_timeout(
        handle,
        MCP_BRIDGE_OUTPUT_DRAIN_TIMEOUT,
        &format!("MCP bridge {name}"),
    )?
}

fn read_bounded_mcp_stderr(mut reader: impl Read) -> io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok((retained, truncated));
        }
        let keep = MCP_BRIDGE_STDERR_LIMIT
            .saturating_sub(retained.len())
            .min(read);
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < read;
    }
}

fn join_mcp_stderr(stderr: &mut Option<McpStderrReader>) -> Result<(Vec<u8>, bool)> {
    crate::join_thread_with_timeout(
        stderr.take().expect("MCP stderr reader should exist"),
        MCP_BRIDGE_OUTPUT_DRAIN_TIMEOUT,
        "MCP bridge stderr",
    )?
    .context("failed to read MCP server stderr")
}

fn stop_mcp_child(child: &mut Child) {
    let _ = crate::terminate_child_process_tree(child, true);
    let _ = child.wait();
}

fn mcp_child_status_result(status: ExitStatus, stderr: (Vec<u8>, bool)) -> Result<()> {
    if !status.success() {
        let diagnostic = String::from_utf8_lossy(&stderr.0);
        let diagnostic = diagnostic.trim();
        if diagnostic.is_empty() {
            bail!("MCP server exited with status {status}");
        }
        let suffix = if stderr.1 { " (stderr truncated)" } else { "" };
        bail!("MCP server exited with status {status}: {diagnostic}{suffix}");
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn exited_mcp_parent_cleans_descendant_held_pipes() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "sleep 30 & exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::configure_child_process_group(&mut command, true);
        let mut child = command.spawn().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut output = Some(std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes)?;
            Ok(())
        }));
        let mut input = Some(std::thread::spawn(|| Ok(())));
        let mut stderr = Some(std::thread::spawn(move || read_bounded_mcp_stderr(stderr)));
        let status = child.wait().unwrap();
        let started = Instant::now();

        handle_exited_mcp_child(&mut child, status, &mut output, &mut input, &mut stderr).unwrap();

        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
