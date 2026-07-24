use crate::McpJsonlBridgeArgs;
use anyhow::{Context, Result, bail};
use prodex_mcp_stdio::{McpMessageFraming, read_mcp_message, write_mcp_message};
use std::io::{self, BufReader, BufWriter};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MCP_BRIDGE_MONITOR_INTERVAL: Duration = Duration::from_millis(10);
const MCP_BRIDGE_OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) fn handle_mcp_jsonl_bridge(args: McpJsonlBridgeArgs) -> Result<()> {
    let mut child = Command::new(&args.command)
        .args(&args.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
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

    monitor_mcp_bridge(&mut child, output, input)
}

fn monitor_mcp_bridge(
    child: &mut Child,
    output: JoinHandle<Result<()>>,
    input: JoinHandle<Result<()>>,
) -> Result<()> {
    let mut output = Some(output);
    let mut input = Some(input);
    loop {
        if let Err(err) = join_finished_mcp_input(&mut input) {
            stop_mcp_child(child);
            return Err(err);
        }

        if output.as_ref().is_some_and(JoinHandle::is_finished) {
            let result = join_mcp_bridge_pump(
                output.take().expect("finished output pump should exist"),
                "output",
            );
            if let Err(err) = result {
                stop_mcp_child(child);
                return Err(err);
            }
            if let Some(status) = poll_mcp_child(child)? {
                join_finished_mcp_input(&mut input)?;
                return mcp_child_status_result(status);
            }
            stop_mcp_child(child);
            bail!("MCP server closed stdout before exiting");
        }

        if let Some(status) = poll_mcp_child(child)? {
            let output = output.take().expect("output pump should exist");
            let deadline = Instant::now() + MCP_BRIDGE_OUTPUT_DRAIN_TIMEOUT;
            while !output.is_finished() && Instant::now() < deadline {
                std::thread::sleep(MCP_BRIDGE_MONITOR_INTERVAL);
            }
            if !output.is_finished() {
                bail!("MCP server stdout remained open after child exited");
            }
            join_mcp_bridge_pump(output, "output")?;
            join_finished_mcp_input(&mut input)?;
            return mcp_child_status_result(status);
        }

        std::thread::sleep(MCP_BRIDGE_MONITOR_INTERVAL);
    }
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
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("MCP bridge {name} thread panicked"))?
}

fn stop_mcp_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn mcp_child_status_result(status: ExitStatus) -> Result<()> {
    if !status.success() {
        bail!("MCP server exited with status {status}");
    }
    Ok(())
}
