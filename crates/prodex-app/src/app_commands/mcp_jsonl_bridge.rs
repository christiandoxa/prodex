use crate::McpJsonlBridgeArgs;
use anyhow::{Context, Result};
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
    let mut child_stdin = Some(BufWriter::new(
        child
            .stdin
            .take()
            .context("failed to capture MCP server stdin")?,
    ));
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

    let mut stdin = BufReader::new(io::stdin().lock());
    while let Some((message, input_framing)) = read_mcp_message(&mut stdin)? {
        if let Ok(mut framing) = framing.lock() {
            *framing = input_framing;
        }
        if let Some(child_stdin) = child_stdin.as_mut() {
            write_mcp_message(child_stdin, &message, McpMessageFraming::JsonLine)?;
        }
    }

    drop(child_stdin.take());
    output
        .join()
        .map_err(|_| anyhow::anyhow!("MCP bridge output thread panicked"))??;
    let _ = child.wait();
    Ok(())
}
