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
