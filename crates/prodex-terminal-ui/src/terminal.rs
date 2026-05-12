use std::env;
use std::fs;
use std::process::Command;

use crate::{CLI_MIN_WIDTH, CLI_WIDTH};

pub fn current_cli_width() -> usize {
    terminal_width_chars()
        .unwrap_or(CLI_WIDTH)
        .max(CLI_MIN_WIDTH)
}

pub fn terminal_size_override_usize(env_key: &str) -> Option<usize> {
    env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

pub fn terminal_dimensions_from_tty() -> Option<(usize, usize)> {
    let tty = fs::File::open("/dev/tty").ok()?;
    let output = Command::new("stty").arg("size").stdin(tty).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let mut parts = text.split_whitespace();
    let rows = parts.next()?.parse::<usize>().ok()?;
    let cols = parts.next()?.parse::<usize>().ok()?;
    Some((rows, cols))
}

pub fn terminal_width_chars() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_COLUMNS")
        .or_else(|| terminal_dimensions_from_tty().map(|(_, cols)| cols))
}

pub fn terminal_height_lines() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_LINES")
        .or_else(|| terminal_size_override_usize("LINES"))
        .or_else(|| terminal_dimensions_from_tty().map(|(rows, _)| rows))
}
