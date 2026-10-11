//! Required-Mojo policy for local rewrite request-header forwarding.
//!
//! Only bounded header names cross the ABI. Rust retains header values,
//! credentials, body bytes, and reqwest request construction.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
#[cfg(test)]
const MAX_HEADER_BYTES: usize = 4_096;

unsafe extern "C" {
    fn prodex_runtime_local_rewrite_header_policy_v1(
        abi_version: i64,
        replacing_auth: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
}

/// Mojo's action for one bounded, non-secret local rewrite header name.
#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalRewriteHeaderPolicy {
    Forward = 0,
    TransportLocal = 1,
    Internal = 2,
    AuthReplaced = 3,
}

impl LocalRewriteHeaderPolicy {
    /// Whether the named request header must be omitted from the upstream request.
    pub fn should_skip(self) -> bool {
        !matches!(self, Self::Forward)
    }
}

pub fn header_policy(
    name: &str,
    replacing_auth: bool,
) -> Result<LocalRewriteHeaderPolicy, MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_local_rewrite_header_policy_v1(
            ABI_VERSION,
            i64::from(replacing_auth),
            if name.is_empty() {
                0
            } else {
                name.as_ptr() as usize as u64
            },
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => match output {
            0 => Ok(LocalRewriteHeaderPolicy::Forward),
            1 => Ok(LocalRewriteHeaderPolicy::TransportLocal),
            2 => Ok(LocalRewriteHeaderPolicy::Internal),
            3 => Ok(LocalRewriteHeaderPolicy::AuthReplaced),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn self_test() -> bool {
    header_policy("User-Agent", false)
        .is_ok_and(|policy| policy == LocalRewriteHeaderPolicy::Forward)
        && header_policy("Connection", false)
            .is_ok_and(|policy| policy == LocalRewriteHeaderPolicy::TransportLocal)
        && header_policy("x-prodex-internal-origin", false)
            .is_ok_and(|policy| policy == LocalRewriteHeaderPolicy::Internal)
        && header_policy("Authorization", true)
            .is_ok_and(|policy| policy == LocalRewriteHeaderPolicy::AuthReplaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_policy_boundary_matrix_keeps_forwarding_and_skip_classes_separate() {
        let transport_headers = [
            "connection",
            "Content-Length",
            " host ",
            "Keep-Alive",
            "proxy-authenticate",
            "Proxy-Authorization",
            "TE",
            "Trailer",
            "Transfer-Encoding",
            "Upgrade",
            "Sec-WebSocket-Key",
        ];
        let internal_headers = ["x-prodex-internal-origin", " X-PRODEX-INTERNAL-Trace "];
        let forwarded_headers = [
            "authorization",
            "chatgpt-account-id",
            "session_id",
            "x-codex-turn-state",
            "User-Agent",
        ];
        let mut checked = 0;
        for name in transport_headers {
            checked += 1;
            assert_eq!(
                header_policy(name, false).unwrap(),
                LocalRewriteHeaderPolicy::TransportLocal,
                "header={name:?}"
            );
        }
        for name in internal_headers {
            checked += 1;
            assert_eq!(
                header_policy(name, false).unwrap(),
                LocalRewriteHeaderPolicy::Internal,
                "header={name:?}"
            );
        }
        for name in forwarded_headers {
            checked += 1;
            assert_eq!(
                header_policy(name, false).unwrap(),
                LocalRewriteHeaderPolicy::Forward,
                "header={name:?}"
            );
        }
        assert_eq!(checked, 18);
        for byte in b"abcdefghijklmnopqrstuvwxyz" {
            let name = format!("{}uthOrization", char::from(*byte));
            assert_eq!(
                header_policy(&name, false).unwrap(),
                LocalRewriteHeaderPolicy::Forward
            );
        }
    }

    #[test]
    fn header_policy_auth_replacement_and_bounds_fail_closed() {
        for name in ["Authorization", "CHATGPT-ACCOUNT-ID"] {
            assert_eq!(
                header_policy(name, true).unwrap(),
                LocalRewriteHeaderPolicy::AuthReplaced
            );
            assert_eq!(
                header_policy(name, false).unwrap(),
                LocalRewriteHeaderPolicy::Forward
            );
        }
        assert_eq!(
            header_policy(&"x".repeat(MAX_HEADER_BYTES + 1), false),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn header_policy_handles_empty_and_malformed_names_without_panicking() {
        assert_eq!(
            header_policy("", false).unwrap(),
            LocalRewriteHeaderPolicy::Forward
        );
        for name in ["\t", "\r\n", "x-prodex-internal", "sec-websocket"] {
            assert_eq!(
                header_policy(name, false).unwrap(),
                LocalRewriteHeaderPolicy::Forward,
                "header={name:?}"
            );
        }
        for name in ["host", "x-prodex-internal-origin", "authorization"] {
            assert!(
                header_policy(name, true).unwrap().should_skip(),
                "header={name:?}"
            );
        }
    }
}
