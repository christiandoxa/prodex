use prodex_mojo_core::rich::{
    RUNTIME_RETRY_AFTER_TEXT_HEADER, RUNTIME_RETRY_AFTER_TEXT_MESSAGE, ascii_casefold_equal_exact,
    runtime_retry_after_text_millis,
};
use std::time::Duration;

#[cfg(test)]
use prodex_mojo_core::rich::{
    RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS, RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS,
    RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS,
};

pub fn runtime_retry_after_from_message(message: &str) -> Option<Duration> {
    runtime_retry_after_text_millis(RUNTIME_RETRY_AFTER_TEXT_MESSAGE, message)
        .expect("Mojo retry-after message parser returned an invalid result")
        .map(Duration::from_millis)
}

pub fn runtime_retry_after_from_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Option<Duration> {
    headers
        .into_iter()
        .filter(|(name, _)| {
            ascii_casefold_equal_exact(name, "retry-after")
                .expect("Mojo Retry-After header-name comparison failed")
        })
        .filter_map(|(_, value)| std::str::from_utf8(value).ok())
        .filter_map(|value| {
            runtime_retry_after_text_millis(RUNTIME_RETRY_AFTER_TEXT_HEADER, value)
                .expect("Mojo Retry-After header parser returned an invalid result")
                .map(Duration::from_millis)
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_retry_after_text_end_to_end_unicode_and_header_maximum() {
        assert_eq!(
            runtime_retry_after_from_message("🧭 Please TRY AGAIN IN\u{2003}1.25MS"),
            Some(Duration::from_millis(2)),
        );
        assert_eq!(
            runtime_retry_after_from_message("try again in 7s; try again in 20s"),
            Some(Duration::from_secs(7)),
        );
        assert_eq!(
            runtime_retry_after_from_headers([
                ("Content-Type", b"12".as_slice()),
                ("retry-after", b" 2 ".as_slice()),
                ("Retry-After", b" 301 ".as_slice()),
                ("RETRY-AFTER", b"invalid".as_slice()),
            ]),
            Some(Duration::from_secs(300)),
        );
        assert_eq!(
            runtime_retry_after_from_headers([
                ("Retry-After", b"0".as_slice()),
                ("Retry-After", b"1.2".as_slice()),
            ]),
            None,
        );
    }

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
                prodex_mojo_core::rich::runtime_retry_after_millis(mode, number)
                    .unwrap()
                    .map(Duration::from_millis),
                expected,
                "number={number} mode={mode}"
            );
        }
    }
}
