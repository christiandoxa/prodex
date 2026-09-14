use std::time::Duration;

const RUNTIME_RETRY_AFTER_CAP: Duration = Duration::from_secs(300);

pub fn runtime_retry_after_from_message(message: &str) -> Option<Duration> {
    let lower = message.to_ascii_lowercase();
    let start = lower.find("try again in")? + "try again in".len();
    runtime_retry_after_duration_token(&lower[start..])
}

pub fn runtime_retry_after_from_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Option<Duration> {
    headers
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .filter_map(|(_, value)| std::str::from_utf8(value).ok())
        .filter_map(runtime_retry_after_header_value)
        .max()
}

fn runtime_retry_after_header_value(value: &str) -> Option<Duration> {
    let seconds = value.trim().parse::<u64>().ok()?;
    (seconds > 0).then(|| Duration::from_secs(seconds).min(RUNTIME_RETRY_AFTER_CAP))
}

fn runtime_retry_after_duration_token(value: &str) -> Option<Duration> {
    let value = value.trim_start();
    let number_len = value
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .count();
    if number_len == 0 {
        return None;
    }
    let number = &value[..number_len];
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let whole = whole.parse::<u128>().ok()?;
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let suffix = value[number_len..].trim_start();
    let millis = if suffix.starts_with("ms") {
        whole.checked_add(u128::from(fraction.bytes().any(|byte| byte != b'0')))?
    } else if suffix.starts_with('s') || suffix.starts_with("second") {
        whole
            .checked_mul(1_000)?
            .checked_add(ceil_fraction_millis(fraction)?)?
    } else {
        return None;
    };
    if millis == 0 {
        return None;
    }
    Some(
        Duration::from_millis(u64::try_from(millis).unwrap_or(u64::MAX))
            .min(RUNTIME_RETRY_AFTER_CAP),
    )
}

fn ceil_fraction_millis(fraction: &str) -> Option<u128> {
    if fraction.is_empty() {
        return Some(0);
    }
    let digits = fraction.as_bytes();
    let mut millis = 0u128;
    for digit in digits.iter().copied().take(3) {
        millis = millis
            .checked_mul(10)?
            .checked_add(u128::from(digit - b'0'))?;
    }
    for _ in digits.len().min(3)..3 {
        millis = millis.checked_mul(10)?;
    }
    if digits.len() > 3 && digits[3..].iter().any(|digit| *digit != b'0') {
        millis = millis.checked_add(1)?;
    }
    Some(millis)
}
