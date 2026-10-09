use crate::MojoError;

const WEBSOCKET_FAILURE_PLAN_FIELD_COUNT: usize = 5;

/// Failure class whose pre-commit state transition is planned by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureKind {
    RateLimited,
    AuthFailed,
    Overloaded,
    LocalSelectionBlocked,
}

/// Side-effect instructions for one pre-commit WebSocket failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureStatePlan {
    pub clear_affinity: bool,
    pub store_last_failure: bool,
    pub last_failure_retryable: bool,
    pub record_rate_limit_failure: bool,
    pub record_overload_failure: bool,
}

unsafe extern "C" {
    fn prodex_runtime_websocket_failure_state_plan_v1(
        failure_kind: i64,
        affinity_releasable: i64,
        output: *mut i64,
    ) -> i64;
}

/// Plans state transitions while Rust retains all WebSocket effects.
pub fn websocket_failure_state_plan(
    failure_kind: WebsocketFailureKind,
    affinity_releasable: bool,
) -> Result<WebsocketFailureStatePlan, MojoError> {
    let failure_kind = match failure_kind {
        WebsocketFailureKind::RateLimited => 0,
        WebsocketFailureKind::AuthFailed => 1,
        WebsocketFailureKind::Overloaded => 2,
        WebsocketFailureKind::LocalSelectionBlocked => 3,
    };
    let mut output = [-1_i64; WEBSOCKET_FAILURE_PLAN_FIELD_COUNT];
    let status = unsafe {
        prodex_runtime_websocket_failure_state_plan_v1(
            failure_kind,
            i64::from(affinity_releasable),
            output.as_mut_ptr(),
        )
    };
    if status != 0 || output.iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WebsocketFailureStatePlan {
        clear_affinity: output[0] == 1,
        store_last_failure: output[1] == 1,
        last_failure_retryable: output[2] == 1,
        record_rate_limit_failure: output[3] == 1,
        record_overload_failure: output[4] == 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [WebsocketFailureKind; 4] = [
        WebsocketFailureKind::RateLimited,
        WebsocketFailureKind::AuthFailed,
        WebsocketFailureKind::Overloaded,
        WebsocketFailureKind::LocalSelectionBlocked,
    ];

    #[test]
    fn failure_state_plan_covers_the_complete_affinity_pressure_matrix() {
        let mut confirmed = 0;
        for kind in KINDS {
            for affinity_releasable in [false, true] {
                let plan = websocket_failure_state_plan(kind, affinity_releasable)
                    .expect("Mojo websocket failure state plan should be valid");
                match kind {
                    WebsocketFailureKind::RateLimited => {
                        assert!(!plan.clear_affinity);
                        assert!(plan.store_last_failure);
                        assert!(!plan.last_failure_retryable);
                        assert!(plan.record_rate_limit_failure);
                        assert!(!plan.record_overload_failure);
                    }
                    WebsocketFailureKind::AuthFailed => {
                        assert_eq!(plan.clear_affinity, affinity_releasable);
                        assert!(plan.store_last_failure);
                        assert!(plan.last_failure_retryable);
                    }
                    WebsocketFailureKind::Overloaded => {
                        assert!(!plan.clear_affinity);
                        assert!(plan.store_last_failure);
                        assert!(!plan.last_failure_retryable);
                        assert!(plan.record_overload_failure);
                        assert!(!plan.record_rate_limit_failure);
                    }
                    WebsocketFailureKind::LocalSelectionBlocked => {
                        assert_eq!(plan.clear_affinity, affinity_releasable);
                        assert!(!plan.store_last_failure);
                    }
                }
                confirmed += 1;
            }
        }
        assert_eq!(confirmed, 8);
    }

    #[test]
    fn repeated_failures_keep_terminal_owner_and_releasable_candidates_distinct() {
        let mut excluded = false;
        let mut retryable_failures = 0;
        for (kind, affinity_releasable) in [
            (WebsocketFailureKind::RateLimited, true),
            (WebsocketFailureKind::RateLimited, true),
            (WebsocketFailureKind::Overloaded, true),
            (WebsocketFailureKind::Overloaded, false),
        ] {
            let plan = websocket_failure_state_plan(kind, affinity_releasable)
                .expect("repeated failure plan should be valid");
            if affinity_releasable {
                excluded = true;
            }
            if affinity_releasable && plan.store_last_failure && !plan.last_failure_retryable {
                retryable_failures += 1;
            }
            if !affinity_releasable {
                assert!(!plan.clear_affinity);
            }
        }
        assert!(excluded);
        assert_eq!(retryable_failures, 3);
    }
}
