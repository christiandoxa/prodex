//! RTK shell-command wrapping for Gemini tool-call arguments.

use crate::translators::tool_args::wrap_json_string_arg_with;

#[path = "rtk/noisy.rs"]
mod noisy;

use self::noisy::{GEMINI_RTK_NOISY_SHELL_COMMANDS, GeminiRtkNoisyShellCommand};

pub(super) fn gemini_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "shell" | "exec_command") {
        return arguments.to_string();
    }
    wrap_json_string_arg_with(
        arguments,
        &["cmd", "command"],
        gemini_rtk_wrapped_shell_command,
    )
}

fn gemini_rtk_wrapped_shell_command(command: &str) -> Option<String> {
    let insert_at = gemini_rtk_insert_index(command)?;
    Some(format!(
        "{}rtk {}",
        &command[..insert_at],
        &command[insert_at..]
    ))
}

fn gemini_rtk_insert_index(command: &str) -> Option<usize> {
    let mut segment_start = 0;
    let mut index = 0;
    while index <= command.len() {
        if index == command.len() {
            return gemini_rtk_insert_index_in_segment(&command[segment_start..])
                .map(|offset| segment_start + offset);
        }
        let rest = &command[index..];
        let separator_len = if rest.starts_with("&&") || rest.starts_with("||") {
            2
        } else if rest.starts_with(';') || rest.starts_with('|') || rest.starts_with('\n') {
            1
        } else {
            0
        };
        if separator_len > 0 {
            if let Some(offset) = gemini_rtk_insert_index_in_segment(&command[segment_start..index])
            {
                return Some(segment_start + offset);
            }
            index += separator_len;
            segment_start = index;
            continue;
        }
        let Some((next_index, _)) = command[index..]
            .char_indices()
            .nth(1)
            .map(|(offset, ch)| (index + offset, ch))
        else {
            index = command.len();
            continue;
        };
        index = next_index;
    }
    None
}

fn gemini_rtk_insert_index_in_segment(segment: &str) -> Option<usize> {
    let mut offset = gemini_skip_shell_whitespace(segment, 0);
    if gemini_shell_token_at(segment, offset).is_some_and(|token| token == "rtk") {
        return None;
    }
    while let Some(token) = gemini_shell_token_at(segment, offset) {
        if !gemini_is_env_assignment(token) {
            break;
        }
        offset += token.len();
        offset = gemini_skip_shell_whitespace(segment, offset);
    }
    let token = gemini_shell_token_at(segment, offset)?;
    let command_name = token.rsplit('/').next().unwrap_or(token);
    let spec = GEMINI_RTK_NOISY_SHELL_COMMANDS
        .iter()
        .find(|spec| spec.command == command_name)?;
    if spec.subcommands.is_empty()
        || gemini_shell_segment_has_subcommand(&segment[offset + token.len()..], spec)
    {
        Some(offset)
    } else {
        None
    }
}

fn gemini_skip_shell_whitespace(segment: &str, mut offset: usize) -> usize {
    while let Some(ch) = segment[offset..].chars().next()
        && ch.is_whitespace()
    {
        offset += ch.len_utf8();
    }
    offset
}

fn gemini_shell_token_at(segment: &str, offset: usize) -> Option<&str> {
    if offset >= segment.len() {
        return None;
    }
    let end = segment[offset..]
        .char_indices()
        .find_map(|(relative, ch)| ch.is_whitespace().then_some(offset + relative))
        .unwrap_or(segment.len());
    (end > offset).then(|| &segment[offset..end])
}

fn gemini_is_env_assignment(token: &str) -> bool {
    let Some((name, value)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !value.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
}

fn gemini_shell_segment_has_subcommand(segment: &str, spec: &GeminiRtkNoisyShellCommand) -> bool {
    segment.split_whitespace().any(|token| {
        let token = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | '{' | '}'));
        spec.subcommands.contains(&token)
    })
}
