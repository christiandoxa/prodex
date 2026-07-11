use anyhow::{Context, Result};
use std::io::{self, Write};

use crate::terminal::current_cli_width;
use crate::text::wrap_text;

pub fn print_stdout_text(message: &str) {
    print!("{message}");
}

pub fn print_stdout_line(message: &str) {
    println!("{message}");
}

pub fn print_blank_line() {
    println!();
}

pub fn print_stderr_line(message: &str) {
    eprintln!("{message}");
}

pub fn print_stderr_prompt(prompt: &str) -> Result<()> {
    eprint!("{prompt}");
    io::stderr().flush().context("failed to flush prompt")
}

pub fn print_wrapped_stderr(message: &str) {
    for line in wrap_text(message, current_cli_width()) {
        print_stderr_line(&line);
    }
}
