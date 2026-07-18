use super::{AppServerBrokerArgs, Result};
use anyhow::Context;
use std::io::{BufRead, BufReader, Write, stdout};
use terminal_ui::print_stdout_line;

#[path = "app_server_broker_live.rs"]
mod live;

pub(crate) fn handle_app_server_broker(args: AppServerBrokerArgs) -> Result<()> {
    if args.experimental_stdio_validate_passthrough {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        let stdout = stdout();
        let passthrough_writer = stdout.lock();
        let stderr = std::io::stderr();
        let diagnostics_writer = stderr.lock();
        run_app_server_broker_stdio_validate_passthrough(
            reader,
            passthrough_writer,
            diagnostics_writer,
        )?;
        return Ok(());
    }

    if args.experimental_stdio_validate {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        let stdout = stdout();
        let writer = stdout.lock();
        run_app_server_broker_stdio_validate(reader, writer)?;
        return Ok(());
    }

    if args.experimental_stdio_passthrough_preview {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        let stdout = stdout();
        let passthrough_writer = stdout.lock();
        let stderr = std::io::stderr();
        let diagnostics_writer = stderr.lock();
        run_app_server_broker_stdio_passthrough_preview(
            reader,
            passthrough_writer,
            diagnostics_writer,
        )?;
        return Ok(());
    }

    if args.experimental_stdio_live {
        return live::run_app_server_broker_process(args.profile.as_deref());
    }

    if args.experimental_stdio {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        let stdout = stdout();
        let writer = stdout.lock();
        run_app_server_broker_stdio_preview(reader, writer)?;
        return Ok(());
    }

    let output = render_app_server_broker_output(args.json)?;
    print_stdout_line(&output)?;
    Ok(())
}

pub(crate) fn render_app_server_broker_output(json: bool) -> Result<String> {
    crate::app_server_broker::app_server_broker_render_output(json)
}

pub(crate) fn run_app_server_broker_stdio_preview<R: BufRead, W: Write>(
    reader: R,
    writer: W,
) -> Result<()> {
    run_app_server_broker_stdio_passthrough_preview(reader, std::io::sink(), writer)
}

pub(crate) fn run_app_server_broker_stdio_validate<R: BufRead, W: Write>(
    reader: R,
    writer: W,
) -> Result<()> {
    crate::app_server_broker::app_server_broker_write_stdio_validate_stream(reader, writer)
        .context("failed to validate app-server broker stdio stream")
}

pub(crate) fn run_app_server_broker_stdio_validate_passthrough<R: BufRead, W: Write, D: Write>(
    reader: R,
    passthrough_writer: W,
    diagnostics_writer: D,
) -> Result<()> {
    crate::app_server_broker::app_server_broker_write_stdio_validate_passthrough_stream(
        reader,
        passthrough_writer,
        diagnostics_writer,
    )
    .context("failed to validate app-server broker stdio passthrough stream")
}

pub(crate) fn run_app_server_broker_stdio_passthrough_preview<R: BufRead, W: Write, D: Write>(
    reader: R,
    passthrough_writer: W,
    diagnostics_writer: D,
) -> Result<()> {
    crate::app_server_broker::app_server_broker_write_stdio_passthrough_preview_stream(
        reader,
        passthrough_writer,
        diagnostics_writer,
    )
    .context("failed to stream app-server broker stdio preview")
}

#[cfg(test)]
pub(crate) fn run_app_server_broker_stdio_live<R: BufRead, W: Write, D: Write>(
    reader: R,
    passthrough_writer: W,
    diagnostics_writer: D,
) -> Result<()> {
    crate::app_server_broker::app_server_broker_write_stdio_live_stream(
        reader,
        passthrough_writer,
        diagnostics_writer,
    )
    .context("failed to run app-server broker stdio live mode")
}
