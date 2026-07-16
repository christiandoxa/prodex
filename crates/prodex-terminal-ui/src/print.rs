use std::io::{self, Write};

use crate::terminal::current_cli_width;
use crate::text::wrap_text;

pub fn print_stdout_text(message: &str) -> io::Result<()> {
    write_text(&mut io::stdout().lock(), message)
}

pub fn print_stdout_line(message: &str) -> io::Result<()> {
    write_line(&mut io::stdout().lock(), message)
}

pub fn print_blank_line() -> io::Result<()> {
    writeln!(io::stdout().lock())
}

pub fn print_stderr_line(message: &str) -> io::Result<()> {
    write_line(&mut io::stderr().lock(), message)
}

pub fn print_stderr_prompt(prompt: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    write_text(&mut stderr, prompt)?;
    stderr.flush()
}

pub fn print_wrapped_stderr(message: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    for line in wrap_text(message, current_cli_width()) {
        write_line(&mut stderr, &line)?;
    }
    Ok(())
}

fn write_text(writer: &mut impl Write, message: &str) -> io::Result<()> {
    writer.write_all(message.as_bytes())
}

fn write_line(writer: &mut impl Write, message: &str) -> io::Result<()> {
    writeln!(writer, "{message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn line_writer_preserves_broken_pipe() {
        let error = write_line(&mut BrokenPipeWriter, "output").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }
}
