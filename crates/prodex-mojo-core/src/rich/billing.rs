use super::*;

pub const GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT: usize = 9;
const GATEWAY_BILLING_SUMMARY_MAX_INPUTS: usize = 100_000;

/// Secret-free numeric references and measurements for one billing-summary row.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GatewayBillingSummaryInput {
    pub bucket_ids: [i64; GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT],
    pub response_status: i64,
    pub response_status_present: i64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub response_bytes: u64,
    pub estimated_cost_microusd: u64,
    pub final_cost_microusd: u64,
    pub created_at_epoch: u64,
    pub reconciled_at_epoch: u64,
    pub reconciled_at_present: i64,
}

/// Numeric billing-summary totals returned for one opaque bucket index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GatewayBillingSummaryBucket {
    pub requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub unreconciled_requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub response_bytes: u64,
    pub estimated_cost_microusd: u64,
    pub final_cost_microusd: u64,
    pub first_created_at_epoch: u64,
    pub first_created_at_present: i64,
    pub last_created_at_epoch: u64,
    pub last_reconciled_at_epoch: u64,
    pub last_reconciled_at_present: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GatewayBillingSummaryResult {
    abi_version: i64,
    buckets_written: i64,
    required_buckets: i64,
    issue_kind: i64,
}

const _: () = {
    assert!(std::mem::size_of::<GatewayBillingSummaryInput>() == 152);
    assert!(std::mem::size_of::<GatewayBillingSummaryBucket>() == 112);
    assert!(std::mem::size_of::<GatewayBillingSummaryResult>() == 32);
};

unsafe extern "C" {
    fn prodex_mojo_rich_gateway_billing_summary_v1(
        abi_version: i64,
        inputs: u64,
        input_count: i64,
        outputs: u64,
        bucket_count: i64,
        result: u64,
    ) -> i64;
}

/// Aggregate bounded billing rows while keeping identifiers in Rust.
pub fn gateway_billing_summary_batch(
    inputs: &[GatewayBillingSummaryInput],
    bucket_count: usize,
) -> Result<Vec<GatewayBillingSummaryBucket>, MojoError> {
    ensure_rich_abi()?;
    if inputs.len() > GATEWAY_BILLING_SUMMARY_MAX_INPUTS
        || bucket_count == 0
        || bucket_count
            > inputs
                .len()
                .saturating_mul(GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT)
                + 1
    {
        return Err(MojoError::InvalidInput);
    }
    let input_count = i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?;
    let bucket_count = i64::try_from(bucket_count).map_err(|_| MojoError::InvalidInput)?;
    let mut outputs = vec![GatewayBillingSummaryBucket::default(); bucket_count as usize];
    let mut result = GatewayBillingSummaryResult::default();
    let status = unsafe {
        prodex_mojo_rich_gateway_billing_summary_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(inputs.as_ptr()),
            input_count,
            mojo_mut_pointer_address(outputs.as_mut_ptr()),
            bucket_count,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 8, result.issue_kind, 0, 0));
    }
    if result.abi_version != RICH_ABI_VERSION
        || result.buckets_written != bucket_count
        || result.required_buckets != bucket_count
    {
        return Err(MojoError::InvalidOutput);
    }
    for bucket in &outputs {
        if !matches!(bucket.first_created_at_present, 0 | 1)
            || !matches!(bucket.last_reconciled_at_present, 0 | 1)
            || bucket
                .successful_requests
                .checked_add(bucket.failed_requests)
                .and_then(|value| value.checked_add(bucket.unreconciled_requests))
                != Some(bucket.requests)
            || (bucket.requests > 0 && bucket.first_created_at_present != 1)
        {
            return Err(MojoError::InvalidOutput);
        }
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::{
        GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT, GatewayBillingSummaryInput,
        gateway_billing_summary_batch,
    };

    fn input(
        bucket_ids: [i64; GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT],
    ) -> GatewayBillingSummaryInput {
        GatewayBillingSummaryInput {
            bucket_ids,
            response_status: 200,
            response_status_present: 1,
            input_tokens: 100,
            output_tokens: 25,
            response_bytes: 512,
            estimated_cost_microusd: 250,
            final_cost_microusd: 500,
            created_at_epoch: 10,
            reconciled_at_epoch: 20,
            reconciled_at_present: 1,
        }
    }

    #[test]
    fn aggregates_opaque_buckets_and_statuses() {
        let mut first = input([0, 1, 2, 3, 4, -1, -1, -1, -1]);
        let mut second = input([0, 1, 2, 3, 4, 5, -1, -1, -1]);
        first.created_at_epoch = 20;
        second.response_status = 500;
        second.response_bytes = 0;
        second.reconciled_at_present = 0;
        let buckets = gateway_billing_summary_batch(&[first, second], 6).unwrap();
        assert_eq!(buckets[0].requests, 2);
        assert_eq!(buckets[0].successful_requests, 1);
        assert_eq!(buckets[0].failed_requests, 1);
        assert_eq!(buckets[0].first_created_at_epoch, 10);
        assert_eq!(buckets[0].last_reconciled_at_epoch, 20);
        assert_eq!(buckets[5].requests, 1);
        assert_eq!(buckets[5].unreconciled_requests, 0);
    }

    #[test]
    fn rejects_invalid_bucket_reference() {
        assert!(
            gateway_billing_summary_batch(&[input([0, 9, -1, -1, -1, -1, -1, -1, -1])], 2).is_err()
        );
    }
}
