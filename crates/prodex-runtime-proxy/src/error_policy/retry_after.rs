use std::time::Duration;

const RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS: i64 = 0;
const RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS: i64 = 1;
const RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS: i64 = 2;

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

fn runtime_retry_after_number(number: &str, mode: i64) -> Option<Duration> {
    prodex_mojo_core::rich::runtime_retry_after_millis(mode, number)
        .expect("Mojo retry-after parser returned an invalid result")
        .map(Duration::from_millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_numeric_policy_matches_expected_values() {
        let cases = [
            (
                ("1", RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS),
                Some(Duration::from_secs(1)),
            ),
            (
                ("300", RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS),
                Some(Duration::from_secs(300)),
            ),
            (
                ("301", RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS),
                Some(Duration::from_secs(300)),
            ),
            (
                (
                    "18446744073709551615",
                    RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS,
                ),
                Some(Duration::from_secs(300)),
            ),
            (
                (
                    "18446744073709551616",
                    RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS,
                ),
                None,
            ),
            (
                ("1", RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS),
                Some(Duration::from_millis(1)),
            ),
            (
                ("1.1", RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS),
                Some(Duration::from_millis(2)),
            ),
            (
                ("0.1", RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS),
                Some(Duration::from_millis(1)),
            ),
            (
                ("11.054", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
                Some(Duration::from_millis(11_054)),
            ),
            (
                ("0.0001", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
                Some(Duration::from_millis(1)),
            ),
            (
                ("0.9991", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
                Some(Duration::from_secs(1)),
            ),
            (
                ("300.1", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
                Some(Duration::from_secs(300)),
            ),
            (
                (
                    "340282366920938463463374607431768211455",
                    RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS,
                ),
                Some(Duration::from_secs(300)),
            ),
            (
                (
                    "340282366920938463463374607431768211456",
                    RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS,
                ),
                None,
            ),
            (
                (
                    "340282366920938463463374607431768211.455",
                    RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
                ),
                Some(Duration::from_secs(300)),
            ),
            (
                (
                    "340282366920938463463374607431768211.456",
                    RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
                ),
                None,
            ),
            (
                (
                    "000000000000000000000001",
                    RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
                ),
                Some(Duration::from_secs(1)),
            ),
            (
                ("1.", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS),
                Some(Duration::from_secs(1)),
            ),
            ((".1", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS), None),
            (("1.2.3", RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS), None),
        ];
        for ((number, mode), expected) in cases {
            assert_eq!(
                runtime_retry_after_number(number, mode),
                expected,
                "number={number} mode={mode}"
            );
        }
    }
}
