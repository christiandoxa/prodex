//! Required-Mojo DeepSeek local rewrite route selection.
//!
//! Rust performs JSON parsing and the provider translation itself. Mojo owns
//! the bounded choice between the native Messages route, the safe automatic
//! chat fallback, and a terminal incompatibility before any upstream response
//! is committed.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

unsafe extern "C" {
    fn prodex_deepseek_local_rewrite_route_v1(
        abi_version: i64,
        mode: i64,
        body_address: u64,
        body_length: i64,
        translation_status: i64,
        reason_address: u64,
        reason_length: i64,
        output_address: u64,
    ) -> i64;
}

/// The provider translation result supplied to the route planner.
#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeepSeekTranslationStatus {
    Lossless = 0,
    Degraded = 1,
    Rejected = 2,
    Unsupported = 3,
}

/// The pre-commit upstream route selected by Mojo.
#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeepSeekLocalRewriteRoute {
    Chat = 0,
    NativeMessages = 1,
    AutoChatFallback = 2,
    Incompatible = 3,
}

/// Select the DeepSeek route before the request is sent upstream.
pub fn route(
    mode: i64,
    body: &[u8],
    translation_status: DeepSeekTranslationStatus,
    rejection_reason: Option<&str>,
) -> Result<DeepSeekLocalRewriteRoute, MojoError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let reason_length = rejection_reason.map_or(0, str::len);
    if reason_length > MAX_BODY_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = -1_i64;
    let status = unsafe {
        prodex_deepseek_local_rewrite_route_v1(
            ABI_VERSION,
            mode,
            if body.is_empty() {
                0
            } else {
                body.as_ptr() as usize as u64
            },
            i64::try_from(body.len()).map_err(|_| MojoError::InvalidInput)?,
            translation_status as i64,
            rejection_reason.map_or(0, |value| value.as_ptr() as usize as u64),
            i64::try_from(reason_length).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => match output {
            0 => Ok(DeepSeekLocalRewriteRoute::Chat),
            1 => Ok(DeepSeekLocalRewriteRoute::NativeMessages),
            2 => Ok(DeepSeekLocalRewriteRoute::AutoChatFallback),
            3 => Ok(DeepSeekLocalRewriteRoute::Incompatible),
            _ => Err(MojoError::InvalidOutput),
        },
        1 | 2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn self_test() -> bool {
    route(
        0,
        br#"{"web_search_options":{}}"#,
        DeepSeekTranslationStatus::Lossless,
        None,
    ) == Ok(DeepSeekLocalRewriteRoute::NativeMessages)
        && route(
            0,
            br#"{"web_search_options":{}}"#,
            DeepSeekTranslationStatus::Rejected,
            Some("Anthropic Messages does not translate chat field `tools`"),
        ) == Ok(DeepSeekLocalRewriteRoute::AutoChatFallback)
        && route(
            3,
            br#"{"web_search_options":{}}"#,
            DeepSeekTranslationStatus::Rejected,
            Some("Anthropic Messages does not translate chat field `tools`"),
        ) == Ok(DeepSeekLocalRewriteRoute::Incompatible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_route_requires_a_translatable_body() {
        assert_eq!(
            route(
                0,
                br#"{"web_search_options":{}}"#,
                DeepSeekTranslationStatus::Degraded,
                None,
            )
            .unwrap(),
            DeepSeekLocalRewriteRoute::NativeMessages
        );
        assert_eq!(
            route(
                0,
                br#"{"web_search_options":{}}"#,
                DeepSeekTranslationStatus::Unsupported,
                Some("provider does not support this request"),
            )
            .unwrap(),
            DeepSeekLocalRewriteRoute::Incompatible
        );
    }

    #[test]
    fn auto_mode_is_the_only_safe_chat_fallback() {
        let reason =
            "Anthropic Messages does not translate web_search_options field `web_search_options`";
        for mode in [3] {
            assert_eq!(
                route(
                    mode,
                    br#"{"web_search_options":{}}"#,
                    DeepSeekTranslationStatus::Rejected,
                    Some(reason),
                )
                .unwrap(),
                DeepSeekLocalRewriteRoute::Incompatible,
                "mode={mode}"
            );
        }
        for mode in [1, 2] {
            assert_eq!(
                route(
                    mode,
                    br#"{"web_search_options":{}}"#,
                    DeepSeekTranslationStatus::Rejected,
                    Some(reason),
                )
                .unwrap(),
                DeepSeekLocalRewriteRoute::Chat,
                "mode={mode}"
            );
        }
        assert_eq!(
            route(
                0,
                br#"{"web_search_options":{}}"#,
                DeepSeekTranslationStatus::Rejected,
                Some(reason),
            )
            .unwrap(),
            DeepSeekLocalRewriteRoute::AutoChatFallback
        );
    }

    #[test]
    fn non_native_requests_stay_on_chat_route() {
        assert_eq!(
            route(
                1,
                br#"{"messages":[{"role":"user","content":"hi"}]}"#,
                DeepSeekTranslationStatus::Rejected,
                Some("translated message must be an object"),
            )
            .unwrap(),
            DeepSeekLocalRewriteRoute::Chat
        );
    }

    #[test]
    fn malformed_and_oversized_inputs_fail_closed() {
        assert_eq!(
            route(0, b"{", DeepSeekTranslationStatus::Lossless, None,),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            route(
                0,
                &vec![b'x'; MAX_BODY_BYTES + 1],
                DeepSeekTranslationStatus::Lossless,
                None,
            ),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            route(
                4,
                br#"{"messages":[]}"#,
                DeepSeekTranslationStatus::Lossless,
                None,
            ),
            Err(MojoError::InvalidInput)
        );
    }
}
