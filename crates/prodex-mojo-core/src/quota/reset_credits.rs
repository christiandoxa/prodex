use super::{QUOTA_MODEL_POLICY_ABI_VERSION, quota_model_policy_status};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QuotaResetCreditExpiry {
    #[default]
    Unavailable,
    Never,
    At(i64),
}

impl QuotaResetCreditExpiry {
    fn parts(self) -> (i64, i64) {
        match self {
            Self::Unavailable => (0, 0),
            Self::Never => (1, 0),
            Self::At(epoch) => (2, epoch),
        }
    }
}

pub struct QuotaResetCreditInput<'a> {
    pub status: &'a str,
    pub expiry: QuotaResetCreditExpiry,
}

unsafe extern "C" {
    fn prodex_quota_reset_credit_expiry_v1(
        abi_version: i64,
        available_count: i64,
        rows_address: u64,
        count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_quota_reset_credit_text_v1(
        abi_version: i64,
        available_count: i64,
        expiry_kind: i64,
        timestamp_address: u64,
        timestamp_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

pub fn quota_reset_credit_expiry(
    available_count: i64,
    credits: &[QuotaResetCreditInput<'_>],
) -> Result<QuotaResetCreditExpiry, crate::MojoError> {
    let rows = credits
        .iter()
        .map(|credit| {
            let (kind, epoch) = credit.expiry.parts();
            Ok([
                credit.status.as_ptr() as usize as i64,
                i64::try_from(credit.status.len()).map_err(|_| crate::MojoError::InvalidInput)?,
                kind,
                epoch,
            ])
        })
        .collect::<Result<Vec<_>, crate::MojoError>>()?;
    let mut output = [-1_i64; 2];
    let status = unsafe {
        prodex_quota_reset_credit_expiry_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            available_count,
            rows.as_ptr() as usize as u64,
            i64::try_from(rows.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    match output {
        [0, 0] => Ok(QuotaResetCreditExpiry::Unavailable),
        [1, 0] => Ok(QuotaResetCreditExpiry::Never),
        [2, epoch] => Ok(QuotaResetCreditExpiry::At(epoch)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn quota_reset_credit_text(
    available_count: i64,
    expiry: QuotaResetCreditExpiry,
    local_timestamp: &str,
) -> Result<String, crate::MojoError> {
    let capacity = local_timestamp
        .len()
        .checked_add(96)
        .ok_or(crate::MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_reset_credit_text_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            available_count,
            expiry.parts().0,
            local_timestamp.as_ptr() as usize as u64,
            i64::try_from(local_timestamp.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| crate::MojoError::InvalidOutput)
}
