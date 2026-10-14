use crate::MojoError;

unsafe extern "C" {
    fn prodex_runtime_session_replay_reset_v1(
        abi: i64,
        previous_response: i64,
        turn_state: i64,
        full_history: i64,
        conflict: i64,
        committed: i64,
    ) -> i64;
    fn prodex_runtime_session_epoch_v1(
        abi: i64,
        operation: i64,
        left_present: i64,
        left: i64,
        right_present: i64,
        right: i64,
    ) -> i64;
}

fn plan(op: i64, left: Option<i64>, right: Option<i64>) -> Result<i64, MojoError> {
    let result = unsafe {
        prodex_runtime_session_epoch_v1(
            1,
            op,
            i64::from(left.is_some()),
            left.unwrap_or(0),
            i64::from(right.is_some()),
            right.unwrap_or(0),
        )
    };
    match result {
        -4 => Err(MojoError::AbiMismatch),
        value if value < 0 => Err(MojoError::InvalidInput),
        value => Ok(value),
    }
}

pub fn merge_session_release_epoch(
    left: Option<i64>,
    right: Option<i64>,
) -> Result<Option<i64>, MojoError> {
    let epoch = plan(0, left, right)?;
    Ok((left.is_some() || right.is_some()).then_some(epoch))
}

pub fn session_binding_survives_release(
    bound_at: i64,
    released_at: Option<i64>,
) -> Result<bool, MojoError> {
    match plan(1, Some(bound_at), released_at)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn session_binding_epoch(now: i64, released_at: Option<i64>) -> Result<i64, MojoError> {
    plan(2, Some(now), released_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_epoch_mojo_preserves_release_across_rebind_and_rejects_stale_owner() {
        assert_eq!(merge_session_release_epoch(None, Some(110)), Ok(Some(110)));
        assert_eq!(
            merge_session_release_epoch(Some(110), Some(100)),
            Ok(Some(110))
        );
        assert_eq!(session_binding_survives_release(110, Some(110)), Ok(false));
        assert_eq!(session_binding_survives_release(109, Some(110)), Ok(false));
        assert_eq!(session_binding_survives_release(111, Some(110)), Ok(true));
        assert_eq!(session_binding_survives_release(-10, None), Ok(true));
        assert_eq!(session_binding_survives_release(-10, Some(110)), Ok(false));
        assert_eq!(session_binding_epoch(110, Some(110)), Ok(111));
        assert_eq!(session_binding_epoch(112, Some(110)), Ok(112));
        assert!(session_binding_epoch(112, Some(i64::MAX)).is_err());
        assert_eq!(
            unsafe { prodex_runtime_session_epoch_v1(9, 0, 1, 10, 1, 20) },
            -4
        );
        assert_eq!(
            unsafe { prodex_runtime_session_epoch_v1(1, 0, 2, 10, 1, 20) },
            -1
        );
    }
}

pub fn session_full_history_reset_allowed(
    previous_response: bool,
    turn_state: bool,
    full_history: bool,
    conflict: bool,
    committed: bool,
) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_session_replay_reset_v1(
            1,
            i64::from(previous_response),
            i64::from(turn_state),
            i64::from(full_history),
            i64::from(conflict),
            i64::from(committed),
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidInput),
    }
}

#[cfg(test)]
#[test]
fn full_history_session_reset_never_weakens_committed_or_opaque_continuations() {
    for previous in [false, true] {
        for turn in [false, true] {
            for history in [false, true] {
                for conflict in [false, true] {
                    for committed in [false, true] {
                        assert_eq!(
                            session_full_history_reset_allowed(
                                previous, turn, history, conflict, committed
                            ),
                            Ok(!previous && !turn && history && conflict && !committed)
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        unsafe { prodex_runtime_session_replay_reset_v1(1, 2, 0, 1, 1, 0) },
        -1
    );
    assert_eq!(
        unsafe { prodex_runtime_session_replay_reset_v1(0, 0, 0, 1, 1, 0) },
        -4
    );
}
