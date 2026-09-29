//! Gemini retry-delay duration parsing helpers.

use prodex_mojo_core::rich::{ascii_casefold_find, ascii_casefold_starts_with};

pub(super) fn gemini_provider_core_duration_ms(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Some(number) = value.strip_suffix("ms") {
        return gemini_provider_core_parse_positive_float(number).map(|millis| {
            let millis = millis.ceil();
            if millis > u64::MAX as f64 {
                u64::MAX
            } else {
                millis as u64
            }
        });
    }
    if let Some(number) = value.strip_suffix('s') {
        return gemini_provider_core_parse_positive_float(number).map(|seconds| {
            let millis = (seconds * 1_000.0).ceil();
            if millis > u64::MAX as f64 {
                u64::MAX
            } else {
                millis as u64
            }
        });
    }
    None
}

pub(super) fn gemini_provider_core_retry_delay_ms_from_message(message: &str) -> Option<u64> {
    ["please retry in ", "suggested retry after "]
        .into_iter()
        .find_map(|marker| {
            let start = ascii_casefold_find(message, marker)
                .expect("Mojo Gemini retry-delay marker search failed")?
                + marker.len();
            gemini_provider_core_duration_token_ms(&message[start..])
        })
}

pub(super) fn gemini_provider_core_retry_after_header_ms(value: &str) -> Option<u64> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|seconds| seconds.saturating_mul(1_000))
        .filter(|delay_ms| *delay_ms > 0)
}

fn gemini_provider_core_parse_positive_float(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn gemini_provider_core_duration_token_ms(value: &str) -> Option<u64> {
    let value = value.trim_start();
    let number_len = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .map(char::len_utf8)
        .sum::<usize>();
    if number_len == 0 {
        return None;
    }
    let number = &value[..number_len];
    let suffix = &value[number_len..];
    if ascii_casefold_starts_with(suffix, "ms")
        .expect("Mojo Gemini retry-delay suffix comparison failed")
    {
        gemini_provider_core_duration_ms(&format!("{number}ms"))
    } else if ascii_casefold_starts_with(suffix, "s")
        .expect("Mojo Gemini retry-delay suffix comparison failed")
    {
        gemini_provider_core_duration_ms(&format!("{number}s"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_message_uses_mojo_casefold_find() {
        assert_eq!(
            gemini_provider_core_retry_delay_ms_from_message(
                "Quota: PLEASE RETRY IN 1.5S after cooldown"
            ),
            Some(1500)
        );
        assert_eq!(
            gemini_provider_core_retry_delay_ms_from_message("Ω SUGGESTED RETRY AFTER 250MS"),
            Some(250)
        );
        assert_eq!(
            gemini_provider_core_retry_delay_ms_from_message("please retry in 2s"),
            Some(2000)
        );
        assert_eq!(
            gemini_provider_core_retry_delay_ms_from_message("retry later"),
            None
        );
    }
}
