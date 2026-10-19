//! Gemini retry-delay duration parsing helpers.
use prodex_mojo_core::json::{GeminiRetryTextKind, gemini_retry_text_delay_ms};

pub(super) fn gemini_provider_core_duration_ms(value: &str) -> Option<u64> {
    gemini_retry_text_delay_ms(GeminiRetryTextKind::Duration, value)
        .expect("Mojo Gemini retry duration failed")
}

pub(super) fn gemini_provider_core_retry_delay_ms_from_message(message: &str) -> Option<u64> {
    gemini_retry_text_delay_ms(GeminiRetryTextKind::Message, message)
        .expect("Mojo Gemini retry message failed")
}

pub(super) fn gemini_provider_core_retry_after_header_ms(value: &str) -> Option<u64> {
    gemini_retry_text_delay_ms(GeminiRetryTextKind::HeaderSeconds, value)
        .expect("Mojo Gemini retry header failed")
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
