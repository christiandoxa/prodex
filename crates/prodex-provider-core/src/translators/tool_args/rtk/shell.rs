//! RTK shell-command insertion helpers.

use super::super::rtk_noisy::{RTK_NOISY_SHELL_COMMANDS, RtkNoisyShellCommand};

pub(super) fn rtk_wrapped_shell_command(command: &str) -> Option<String> {
    let insert_at = rtk_insert_index(command)?;
    Some(format!(
        "{}rtk {}",
        &command[..insert_at],
        &command[insert_at..]
    ))
}

pub(crate) fn rtk_prefixed_noisy_shell_command(command: &str) -> Option<String> {
    if command.trim_start().starts_with("rtk ") || rtk_insert_index(command).is_none() {
        None
    } else {
        Some(format!("rtk {command}"))
    }
}

fn rtk_insert_index(command: &str) -> Option<usize> {
    let mut segment_start = 0;
    let mut index = 0;
    while index <= command.len() {
        if index == command.len() {
            return rtk_insert_index_in_segment(&command[segment_start..])
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
            if let Some(offset) = rtk_insert_index_in_segment(&command[segment_start..index]) {
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

fn rtk_insert_index_in_segment(segment: &str) -> Option<usize> {
    let mut offset = skip_shell_whitespace(segment, 0);
    if shell_token_at(segment, offset).is_some_and(|token| token == "rtk") {
        return None;
    }
    while let Some(token) = shell_token_at(segment, offset) {
        if !is_env_assignment(token) {
            break;
        }
        offset += token.len();
        offset = skip_shell_whitespace(segment, offset);
    }
    let token = shell_token_at(segment, offset)?;
    let command_name = token.rsplit('/').next().unwrap_or(token);
    let spec = RTK_NOISY_SHELL_COMMANDS
        .iter()
        .find(|spec| spec.command == command_name)?;
    if spec.subcommands.is_empty()
        || shell_segment_has_subcommand(&segment[offset + token.len()..], spec)
    {
        Some(offset)
    } else {
        None
    }
}

fn skip_shell_whitespace(segment: &str, mut offset: usize) -> usize {
    while let Some(ch) = segment[offset..].chars().next()
        && ch.is_whitespace()
    {
        offset += ch.len_utf8();
    }
    offset
}

fn shell_token_at(segment: &str, offset: usize) -> Option<&str> {
    if offset >= segment.len() {
        return None;
    }
    let end = segment[offset..]
        .char_indices()
        .find_map(|(relative, ch)| ch.is_whitespace().then_some(offset + relative))
        .unwrap_or(segment.len());
    (end > offset).then(|| &segment[offset..end])
}

fn is_env_assignment(token: &str) -> bool {
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

fn shell_segment_has_subcommand(segment: &str, spec: &RtkNoisyShellCommand) -> bool {
    segment.split_whitespace().any(|token| {
        let token = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | '{' | '}'));
        spec.subcommands.contains(&token)
    })
}
