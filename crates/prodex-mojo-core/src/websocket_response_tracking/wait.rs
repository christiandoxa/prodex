//! Checked transport-liveness planning, independent of response commitment.
use crate::MojoError;

unsafe extern "C" {
    fn prodex_websocket_response_wait_plan_v1(
        abi_version: i64,
        upstream_elapsed_ms: u64,
        upstream_timeout_ms: u64,
        heartbeat_elapsed_ms: u64,
        output_address: u64,
        output_count: i64,
    ) -> i64;
}

#[derive(Debug, PartialEq, Eq)]
pub struct WebsocketResponseWaitPlan {
    pub expired: bool,
    pub heartbeat: bool,
    pub wait_ms: u64,
}

pub fn response_wait_plan(
    upstream_elapsed_ms: u64,
    upstream_timeout_ms: u64,
    heartbeat_elapsed_ms: u64,
) -> Result<WebsocketResponseWaitPlan, MojoError> {
    let mut output = [u64::MAX; 3];
    let status = unsafe {
        prodex_websocket_response_wait_plan_v1(
            1,
            upstream_elapsed_ms,
            upstream_timeout_ms,
            heartbeat_elapsed_ms,
            output.as_mut_ptr() as usize as u64,
            3,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match output {
        [1, 0, 0] => Ok(WebsocketResponseWaitPlan {
            expired: true,
            heartbeat: false,
            wait_ms: 0,
        }),
        [0, heartbeat @ 0..=1, wait_ms @ 1..=1000] => Ok(WebsocketResponseWaitPlan {
            expired: false,
            heartbeat: heartbeat == 1,
            wait_ms,
        }),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_plan_preserves_upstream_deadline_while_scheduling_transport_heartbeats() {
        for (elapsed, timeout, heartbeat_elapsed, expected) in [
            (0, 8000, 0, (false, false, 1000)),
            (1000, 8000, 1000, (false, true, 1000)),
            (7900, 8000, 0, (false, false, 100)),
            (7999, 8000, 1000, (false, true, 1)),
            (8000, 8000, 1000, (true, false, 0)),
            (100, 300000, 999, (false, false, 1)),
            (u64::MAX, u64::MAX, 0, (true, false, 0)),
            (0, u64::MAX, u64::MAX, (false, true, 1000)),
        ] {
            let plan = response_wait_plan(elapsed, timeout, heartbeat_elapsed).unwrap();
            assert_eq!((plan.expired, plan.heartbeat, plan.wait_ms), expected);
        }
    }

    #[test]
    fn wait_plan_abi_rejects_invalid_inputs_without_output_mutation() {
        let mut out = [19_u64; 3];
        let ptr = out.as_mut_ptr() as usize as u64;
        unsafe {
            assert_eq!(
                prodex_websocket_response_wait_plan_v1(2, 0, 8, 0, ptr, 3),
                4
            );
            assert_eq!(
                prodex_websocket_response_wait_plan_v1(1, 0, 0, 0, ptr, 3),
                1
            );
            assert_eq!(prodex_websocket_response_wait_plan_v1(1, 0, 8, 0, 0, 3), 1);
            assert_eq!(
                prodex_websocket_response_wait_plan_v1(1, 0, 8, 0, ptr, 2),
                1
            );
        }
        assert_eq!(out, [19; 3]);
    }
}
