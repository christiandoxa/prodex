use super::*;
use std::borrow::Cow;

pub fn smart_context_normalize_volatile_command_output(text: &str) -> Cow<'_, str> {
    let mut normalized = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0usize;

    while index < text.len() {
        let rest = &text[index..];
        let previous = smart_context_previous_char(text, index);

        if let Some(len) = smart_context_ansi_escape_len(rest) {
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_temp_path_len(rest) {
            normalized.push_str("<tmp-path>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_timestamp_len(rest, previous) {
            normalized.push_str("<timestamp>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_progress_counter_len(rest, previous) {
            normalized.push_str("<progress>");
            index += len;
            changed = true;
            continue;
        }
        if let Some((len, replacement)) =
            smart_context_labeled_random_id_replacement(rest, previous)
        {
            normalized.push_str(&replacement);
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_uuid_len(rest, previous) {
            normalized.push_str("<id>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_duration_len(rest, previous) {
            normalized.push_str("<duration>");
            index += len;
            changed = true;
            continue;
        }

        let ch = rest.chars().next().expect("index is within string");
        normalized.push(ch);
        index += ch.len_utf8();
    }

    if changed {
        Cow::Owned(normalized)
    } else {
        Cow::Borrowed(text)
    }
}

pub fn smart_context_normalize_volatile_static_context(text: &str) -> Cow<'_, str> {
    let mut normalized = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0usize;

    while index < text.len() {
        let rest = &text[index..];
        let previous = smart_context_previous_char(text, index);

        if let Some(len) = smart_context_ansi_escape_len(rest) {
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_temp_path_len(rest) {
            normalized.push_str("<tmp-path>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_timestamp_len(rest, previous) {
            normalized.push_str("<timestamp>");
            index += len;
            changed = true;
            continue;
        }
        if let Some((len, replacement)) =
            smart_context_labeled_random_id_replacement(rest, previous)
        {
            normalized.push_str(&replacement);
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_uuid_len(rest, previous) {
            normalized.push_str("<id>");
            index += len;
            changed = true;
            continue;
        }

        let ch = rest.chars().next().expect("index is within string");
        normalized.push(ch);
        index += ch.len_utf8();
    }

    if changed {
        Cow::Owned(normalized)
    } else {
        Cow::Borrowed(text)
    }
}

pub(in crate::smart_context) fn smart_context_previous_char(
    text: &str,
    index: usize,
) -> Option<char> {
    if index == 0 {
        None
    } else {
        text[..index].chars().next_back()
    }
}

pub(in crate::smart_context) fn smart_context_ansi_escape_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first().copied()? != 0x1b {
        return None;
    }

    match bytes.get(1).copied() {
        Some(b'[') => bytes
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, byte)| (0x40u8..=0x7e).contains(*byte))
            .map(|(index, _)| index + 1)
            .or(Some(bytes.len())),
        Some(b']') => {
            let mut index = 2usize;
            while index < bytes.len() {
                if bytes[index] == 0x07 {
                    return Some(index + 1);
                }
                if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
                    return Some(index + 2);
                }
                index += 1;
            }
            Some(bytes.len())
        }
        Some(_) => Some(2.min(bytes.len())),
        None => Some(1),
    }
}

pub(in crate::smart_context) fn smart_context_temp_path_len(text: &str) -> Option<usize> {
    for prefix in [
        "/tmp/",
        "/var/tmp/",
        "/private/tmp/",
        "/var/folders/",
        "$TMPDIR/",
        "%TEMP%\\",
        "%TMP%\\",
    ] {
        if text.starts_with(prefix) {
            return Some(smart_context_path_token_len(text));
        }
    }

    let bytes = text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        let token_len = smart_context_path_token_len(text);
        let token = text[..token_len].replace('/', "\\").to_ascii_lowercase();
        if token.contains("\\appdata\\local\\temp\\") || token.starts_with("c:\\temp\\") {
            return Some(token_len);
        }
    }

    None
}

pub(in crate::smart_context) fn smart_context_path_token_len(text: &str) -> usize {
    text.char_indices()
        .find(|(index, ch)| *index > 0 && smart_context_path_token_delimiter(*ch))
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

pub(in crate::smart_context) fn smart_context_path_token_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_control()
        || matches!(
            ch,
            '"' | '\'' | '`' | '<' | '>' | '|' | '(' | ')' | '[' | ']' | '{' | '}'
        )
}

pub(in crate::smart_context) fn smart_context_timestamp_len(
    text: &str,
    previous: Option<char>,
) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let bytes = text.as_bytes();
    if bytes.len() < 16
        || !smart_context_ascii_digits(bytes, 0, 4)
        || bytes[4] != b'-'
        || !smart_context_ascii_digits(bytes, 5, 2)
        || bytes[7] != b'-'
        || !smart_context_ascii_digits(bytes, 8, 2)
        || !matches!(bytes[10], b'T' | b' ')
        || !smart_context_ascii_digits(bytes, 11, 2)
        || bytes[13] != b':'
        || !smart_context_ascii_digits(bytes, 14, 2)
    {
        return None;
    }

    let mut index = 16usize;
    if bytes.get(index) == Some(&b':') {
        if !smart_context_ascii_digits(bytes, index + 1, 2) {
            return None;
        }
        index += 3;
        if bytes.get(index) == Some(&b'.') {
            let fraction_start = index + 1;
            index = fraction_start;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            if index == fraction_start {
                return None;
            }
        }
    }

    if bytes.get(index) == Some(&b'Z') {
        index += 1;
    } else if matches!(bytes.get(index), Some(b'+') | Some(b'-')) {
        if !smart_context_ascii_digits(bytes, index + 1, 2) {
            return None;
        }
        index += 3;
        if bytes.get(index) == Some(&b':') {
            if !smart_context_ascii_digits(bytes, index + 1, 2) {
                return None;
            }
            index += 3;
        } else if smart_context_ascii_digits(bytes, index, 2) {
            index += 2;
        }
    }

    smart_context_after_token_boundary(text, index).then_some(index)
}

pub(in crate::smart_context) fn smart_context_progress_counter_len(
    text: &str,
    previous: Option<char>,
) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    smart_context_percent_progress_len(text)
        .or_else(|| smart_context_slash_progress_len(text))
        .or_else(|| smart_context_of_progress_len(text))
}

pub(in crate::smart_context) fn smart_context_percent_progress_len(text: &str) -> Option<usize> {
    let (mut index, _) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(index) == Some(&b'.') {
        let fraction_start = index + 1;
        index = fraction_start;
        while text.as_bytes().get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if text.as_bytes().get(index) != Some(&b'%') {
        return None;
    }
    index += 1;
    smart_context_after_token_boundary(text, index).then_some(index)
}

pub(in crate::smart_context) fn smart_context_slash_progress_len(text: &str) -> Option<usize> {
    let (left_end, left) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(left_end) != Some(&b'/') {
        return None;
    }
    let (right_end, right) = smart_context_parse_unsigned_ascii_int(text, left_end + 1)?;
    if left > right || right == 0 {
        return None;
    }
    smart_context_after_token_boundary(text, right_end).then_some(right_end)
}

pub(in crate::smart_context) fn smart_context_of_progress_len(text: &str) -> Option<usize> {
    let (left_end, left) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    let mut index = smart_context_skip_ascii_spaces(text, left_end);
    if !text[index..].starts_with("of") {
        return None;
    }
    index += 2;
    if !text
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }
    index = smart_context_skip_ascii_spaces(text, index);
    let (right_end, right) = smart_context_parse_unsigned_ascii_int(text, index)?;
    if left > right || right == 0 {
        return None;
    }
    smart_context_after_token_boundary(text, right_end).then_some(right_end)
}

pub(in crate::smart_context) fn smart_context_labeled_random_id_replacement(
    text: &str,
    previous: Option<char>,
) -> Option<(usize, String)> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let mut separator_index = None;
    for (index, ch) in text.char_indices().take(64) {
        if matches!(ch, ':' | '=') {
            separator_index = Some(index);
            break;
        }
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ' ')) {
            return None;
        }
    }
    let separator_index = separator_index?;
    let key = smart_context_static_context_noise_key(&text[..separator_index]);
    if !smart_context_random_id_key_is_volatile(&key) {
        return None;
    }

    let mut value_start = smart_context_skip_ascii_spaces(text, separator_index + 1);
    if matches!(text.as_bytes().get(value_start), Some(b'"') | Some(b'\'')) {
        value_start += 1;
    }
    let value_len = smart_context_random_id_value_len(&text[value_start..])?;
    let value_end = value_start + value_len;
    let value = &text[value_start..value_end];
    if !smart_context_random_id_value_looks_volatile_for_key(&key, value) {
        return None;
    }

    let mut replacement = String::with_capacity(value_start + 4);
    replacement.push_str(&text[..value_start]);
    replacement.push_str("<id>");
    Some((value_end, replacement))
}

pub(in crate::smart_context) fn smart_context_uuid_len(
    text: &str,
    previous: Option<char>,
) -> Option<usize> {
    if !smart_context_token_boundary(previous) || text.len() < 36 {
        return None;
    }
    let candidate = &text[..36];
    if !smart_context_uuid_token_exact(candidate) || !smart_context_after_token_boundary(text, 36) {
        return None;
    }
    Some(36)
}

pub(in crate::smart_context) fn smart_context_duration_len(
    text: &str,
    previous: Option<char>,
) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let (mut index, _) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(index) == Some(&b'.') {
        let fraction_start = index + 1;
        index = fraction_start;
        while text.as_bytes().get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    index = smart_context_skip_ascii_spaces(text, index);
    let unit_len = smart_context_duration_unit_len(&text[index..])?;
    let end = index + unit_len;
    smart_context_after_token_boundary(text, end).then_some(end)
}

pub(in crate::smart_context) fn smart_context_duration_unit_len(text: &str) -> Option<usize> {
    for unit in [
        "milliseconds",
        "millisecond",
        "microseconds",
        "microsecond",
        "nanoseconds",
        "nanosecond",
        "seconds",
        "second",
        "minutes",
        "minute",
        "hours",
        "hour",
        "msecs",
        "msec",
        "usecs",
        "usec",
        "nsecs",
        "nsec",
        "secs",
        "sec",
        "mins",
        "min",
        "hrs",
        "hr",
        "ms",
        "us",
        "ns",
        "s",
        "m",
        "h",
    ] {
        if smart_context_ascii_case_prefix(text, unit) {
            return Some(unit.len());
        }
    }
    None
}

pub(in crate::smart_context) fn smart_context_random_id_key_is_volatile(key: &str) -> bool {
    matches!(
        key,
        "request id"
            | "x request id"
            | "trace id"
            | "run id"
            | "session id"
            | "conversation id"
            | "turn id"
            | "span id"
            | "correlation id"
            | "invocation id"
            | "execution id"
            | "operation id"
            | "job id"
            | "build id"
            | "uuid"
            | "id"
    )
}

pub(in crate::smart_context) fn smart_context_random_id_value_len(text: &str) -> Option<usize> {
    let len = text
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':')))
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    (len > 0).then_some(len)
}

pub(in crate::smart_context) fn smart_context_random_id_value_looks_volatile_for_key(
    key: &str,
    value: &str,
) -> bool {
    if smart_context_uuid_token_exact(value) {
        return true;
    }
    if key == "id" && value.len() < 20 {
        return false;
    }
    if value.len() < 12 {
        return false;
    }

    let mut alpha = false;
    let mut digit = false;
    let mut hex_like = true;
    let mut entropy_marks = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_alphabetic() {
            alpha = true;
            if !ch.is_ascii_hexdigit() {
                hex_like = false;
            }
        } else if ch.is_ascii_digit() {
            digit = true;
        } else if matches!(ch, '_' | '-' | '.' | ':') {
            entropy_marks += 1;
        } else {
            return false;
        }
    }

    (hex_like && value.len() >= 16) || (alpha && digit && (value.len() >= 16 || entropy_marks > 0))
}

pub(in crate::smart_context) fn smart_context_uuid_token_exact(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub(in crate::smart_context) fn smart_context_parse_unsigned_ascii_int(
    text: &str,
    start: usize,
) -> Option<(usize, u64)> {
    let bytes = text.as_bytes();
    let mut index = start;
    let mut value = 0u64;
    let mut digits = 0usize;
    while let Some(byte) = bytes.get(index).copied() {
        if !byte.is_ascii_digit() {
            break;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'));
        index += 1;
        digits += 1;
    }
    (digits > 0).then_some((index, value))
}

pub(in crate::smart_context) fn smart_context_skip_ascii_spaces(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

pub(in crate::smart_context) fn smart_context_ascii_digits(
    bytes: &[u8],
    start: usize,
    len: usize,
) -> bool {
    bytes
        .get(start..start.saturating_add(len))
        .is_some_and(|value| value.iter().all(u8::is_ascii_digit))
}

pub(in crate::smart_context) fn smart_context_ascii_case_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
}

pub(in crate::smart_context) fn smart_context_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/')))
}

pub(in crate::smart_context) fn smart_context_after_token_boundary(
    text: &str,
    index: usize,
) -> bool {
    text[index..]
        .chars()
        .next()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/')))
}
