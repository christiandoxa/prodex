use crate::McpJsonlBridgeArgs;
use anyhow::{Context, Result, bail};
use prodex_mcp_stdio::{McpMessageFraming, read_mcp_message, write_mcp_message};
use std::io::{self, BufReader, BufWriter};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

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

    let status = child.wait().context("failed to wait for MCP server")?;
    output
        .join()
        .map_err(|_| anyhow::anyhow!("MCP bridge output thread panicked"))??;
    if input.is_finished() {
        input
            .join()
            .map_err(|_| anyhow::anyhow!("MCP bridge input thread panicked"))??;
    }
    if !status.success() {
        bail!("MCP server exited with status {status}");
    }
    Ok(())
}
