use std::time::Duration;

#[cfg(any(not(feature = "mojo"), test))]
const RUNTIME_RETRY_AFTER_CAP: Duration = Duration::from_secs(300);
const RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS: i64 = 0;
const RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS: i64 = 1;
const RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS: i64 = 2;

#[cfg(feature = "mojo")]
const _: () = {
    assert!(
        RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS
            == prodex_mojo_core::rich::RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS
    );
    assert!(
        RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS
            == prodex_mojo_core::rich::RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS
    );
    assert!(
        RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS
            == prodex_mojo_core::rich::RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS
    );
};

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
    runtime_retry_after_number(value.trim(), RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS)
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
    let suffix = value[number_len..].trim_start();
    let mode = if suffix.starts_with("ms") {
        RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS
    } else if suffix.starts_with('s') || suffix.starts_with("second") {
        RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS
    } else {
        return None;
    };
    runtime_retry_after_number(number, mode)
}

#[cfg(feature = "mojo")]
fn runtime_retry_after_number(number: &str, mode: i64) -> Option<Duration> {
    prodex_mojo_core::rich::runtime_retry_after_millis(mode, number)
        .expect("Mojo retry-after parser returned an invalid result")
        .map(Duration::from_millis)
}

#[cfg(not(feature = "mojo"))]
fn runtime_retry_after_number(number: &str, mode: i64) -> Option<Duration> {
    runtime_retry_after_number_rust(number, mode)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_retry_after_number_rust(number: &str, mode: i64) -> Option<Duration> {
    if mode == RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS {
        let seconds = number.parse::<u64>().ok()?;
        return (seconds > 0).then(|| Duration::from_secs(seconds).min(RUNTIME_RETRY_AFTER_CAP));
    }

    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let whole = whole.parse::<u128>().ok()?;
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let millis = if mode == RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS {
        whole.checked_add(u128::from(fraction.bytes().any(|byte| byte != b'0')))?
    } else if mode == RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS {
        whole
            .checked_mul(1_000)?
            .checked_add(ceil_fraction_millis_rust(fraction)?)?
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

#[cfg(any(not(feature = "mojo"), test))]
fn ceil_fraction_millis_rust(fraction: &str) -> Option<u128> {
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn mojo_retry_after_numeric_policy_matches_rust_oracle() {
        let cases = [
            ("1", RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS),
            ("300", RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS),
            ("301", RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS),
            (
                "18446744073709551615",
                RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS,
            ),
            (
                "18446744073709551616",
                RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS,
            ),
            ("1", RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS),
            ("1.1", RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS),
            ("0.1", RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS),
            ("11.054", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
            ("0.0001", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
            ("0.9991", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
            ("300.1", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
            (
                "340282366920938463463374607431768211455",
                RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS,
            ),
            (
                "340282366920938463463374607431768211456",
                RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS,
            ),
            (
                "340282366920938463463374607431768211.455",
                RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
            ),
            (
                "340282366920938463463374607431768211.456",
                RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
            ),
            (
                "000000000000000000000001",
                RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
            ),
            ("1.", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
            (".1", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
            ("1.2.3", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
        ];
        for (number, mode) in cases {
            assert_eq!(
                runtime_retry_after_number(number, mode),
                runtime_retry_after_number_rust(number, mode),
                "number={number} mode={mode}"
            );
        }
    }
}
