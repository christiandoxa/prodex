use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationPolicy {
    Redeem = 0,
    YesNo = 1,
}

unsafe extern "C" {
    fn prodex_confirmation_policy_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
        default_value: i64,
    ) -> i64;
}

pub fn confirmation_value(
    policy: ConfirmationPolicy,
    input: &str,
    default: bool,
) -> Result<Option<bool>, MojoError> {
    let length = i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_confirmation_policy_v1(
            ABI_VERSION,
            policy as i64,
            input.as_ptr() as usize as u64,
            length,
            i64::from(default),
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Timestamp facts for the two manual-redemption quota windows.
pub struct ManualRedeemResetInput {
    pub primary_reset_at: Option<i64>,
    pub weekly_reset_at: Option<i64>,
    pub now: i64,
    pub near_reset_seconds: i64,
}

/// The selected window retains its original host timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualRedeemResetWindow {
    Primary { reset_at: i64 },
    Weekly { reset_at: i64 },
}

unsafe extern "C" {
    fn prodex_manual_redeem_reset_v1(
        version: i64,
        primary_present: i64,
        primary_reset: i64,
        weekly_present: i64,
        weekly_reset: i64,
        now: i64,
        near_seconds: i64,
    ) -> i64;
}

/// Selects the first nearest eligible reset through the compiled manual-confirmation policy.
pub fn manual_redeem_reset_window(
    input: ManualRedeemResetInput,
) -> Result<Option<ManualRedeemResetWindow>, MojoError> {
    let selected = unsafe {
        prodex_manual_redeem_reset_v1(
            ABI_VERSION,
            i64::from(input.primary_reset_at.is_some()),
            input.primary_reset_at.unwrap_or_default(),
            i64::from(input.weekly_reset_at.is_some()),
            input.weekly_reset_at.unwrap_or_default(),
            input.now,
            input.near_reset_seconds,
        )
    };
    match selected {
        -2 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        0 => Ok(None),
        1 => input
            .primary_reset_at
            .map(|reset_at| Some(ManualRedeemResetWindow::Primary { reset_at }))
            .ok_or(MojoError::InvalidOutput),
        2 => input
            .weekly_reset_at
            .map(|reset_at| Some(ManualRedeemResetWindow::Weekly { reset_at }))
            .ok_or(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "confirmation_policy_reset_tests.rs"]
mod reset_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_policy_preserves_redeem_and_default_semantics() {
        assert_eq!(
            confirmation_value(ConfirmationPolicy::Redeem, "", true).unwrap(),
            Some(false)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::Redeem, " YES ", false).unwrap(),
            Some(true)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::Redeem, "N", true).unwrap(),
            Some(false)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::Redeem, "maybe", false).unwrap(),
            None
        );

        assert_eq!(
            confirmation_value(ConfirmationPolicy::YesNo, "  ", true).unwrap(),
            Some(true)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::YesNo, "", false).unwrap(),
            Some(false)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::YesNo, "yEs", false).unwrap(),
            Some(true)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::YesNo, "NO", true).unwrap(),
            Some(false)
        );
        assert_eq!(
            confirmation_value(ConfirmationPolicy::YesNo, "wat", false).unwrap(),
            None
        );
    }
}
