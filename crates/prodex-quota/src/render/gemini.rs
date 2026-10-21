use super::*;
use prodex_mojo_core::quota::GeminiBucketNumericOutput;

fn gemini_numeric_batch(buckets: &[GeminiQuotaBucket]) -> Vec<GeminiBucketNumericOutput> {
    // ponytail: 1,024-row ABI batches; raise only with a versioned ABI/capacity review.
    buckets
        .chunks(prodex_mojo_core::quota::QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT)
        .flat_map(|batch| {
            crate::mojo::gemini_bucket_numeric_batch(batch)
                .unwrap_or_else(|error| panic!("Mojo Gemini quota numeric batch failed: {error:?}"))
        })
        .collect()
}

fn gemini_bucket_label(bucket: &GeminiQuotaBucket) -> String {
    prodex_mojo_core::quota::quota_gemini_bucket_label(
        bucket.model_id.as_deref(),
        bucket.token_type.as_deref(),
    )
    .expect("Mojo Gemini bucket label policy returned invalid output")
}

fn gemini_display(info: &GeminiQuotaInfo) -> prodex_mojo_core::quota::GeminiQuotaDisplay {
    let numeric = gemini_numeric_batch(&info.buckets);
    prodex_mojo_core::quota::quota_gemini_display(&numeric)
        .expect("Mojo Gemini quota display policy returned invalid output")
}

pub(super) fn gemini_main_remaining_percent(info: &GeminiQuotaInfo) -> Option<i64> {
    gemini_display(info).remaining_percent
}

pub fn gemini_quota_is_ready(info: &GeminiQuotaInfo) -> bool {
    gemini_display(info).ready
}

pub fn format_gemini_quota_status(info: &GeminiQuotaInfo) -> String {
    gemini_display(info).status
}

pub(super) fn format_gemini_bucket_summaries(info: &GeminiQuotaInfo) -> Vec<String> {
    gemini_numeric_batch(&info.buckets)
        .into_iter()
        .zip(&info.buckets)
        .map(|(numeric, bucket)| {
            let label = gemini_bucket_label(bucket);
            prodex_mojo_core::quota::quota_gemini_bucket_summary(&label, numeric)
                .expect("Mojo Gemini bucket summary policy returned invalid output")
        })
        .collect()
}

pub fn format_gemini_main_quota(info: &GeminiQuotaInfo) -> String {
    gemini_display(info).main
}

pub(super) fn gemini_reset_epoch(info: &GeminiQuotaInfo) -> Option<i64> {
    info.buckets
        .iter()
        .filter_map(|bucket| bucket.reset_time.as_deref())
        .filter_map(parse_gemini_reset_time)
        .min()
}

pub fn format_gemini_reset_summary(info: &GeminiQuotaInfo) -> Option<String> {
    let reset = info
        .buckets
        .iter()
        .filter_map(|bucket| bucket.reset_time.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .min_by_key(|value| parse_gemini_reset_time(value).unwrap_or(i64::MAX))?;
    Some(reset.to_string())
}

fn parse_gemini_reset_time(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|datetime| datetime.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_counter_text_preserves_numeric_and_rendered_outputs() {
        use prodex_mojo_core::quota::{GeminiBucketNumericInput, GeminiRemainingAmount};

        let counters = [
            None,
            Some(""),
            Some("bad"),
            Some("　+42\t"),
            Some("0"),
            Some("-2"),
            Some("-9223372036854775808"),
            Some("9223372036854775807"),
            Some("9223372036854775808"),
            Some("4 2"),
        ];
        let buckets = counters
            .iter()
            .map(|amount| GeminiQuotaBucket {
                remaining_amount: amount.map(str::to_string),
                remaining_fraction: Some(0.5),
                reset_time: None,
                token_type: None,
                model_id: None,
            })
            .collect::<Vec<_>>();
        let reference_inputs = counters
            .iter()
            .map(|amount| GeminiBucketNumericInput {
                remaining_amount: match amount {
                    None => GeminiRemainingAmount::Absent,
                    Some(value) => value
                        .trim()
                        .parse::<i64>()
                        .map(GeminiRemainingAmount::Parsed)
                        .unwrap_or(GeminiRemainingAmount::Invalid),
                },
                remaining_fraction: Some(0.5),
            })
            .collect::<Vec<_>>();
        let reference =
            prodex_mojo_core::quota::gemini_bucket_numeric_batch(&reference_inputs).unwrap();
        assert_eq!(
            crate::mojo::gemini_bucket_numeric_batch(&buckets).unwrap(),
            reference
        );
        assert_eq!(reference[0].remaining, Some(50));
        assert_eq!(reference[1].remaining, None);
        assert_eq!(reference[3].remaining, Some(42));
        let display = prodex_mojo_core::quota::quota_gemini_display(&reference).unwrap();
        let expected_summaries = reference
            .iter()
            .zip(&buckets)
            .map(|(numeric, bucket)| {
                let label = prodex_mojo_core::quota::quota_gemini_bucket_label(
                    bucket.model_id.as_deref(),
                    bucket.token_type.as_deref(),
                )
                .unwrap();
                prodex_mojo_core::quota::quota_gemini_bucket_summary(&label, *numeric).unwrap()
            })
            .collect::<Vec<_>>();
        let info = GeminiQuotaInfo {
            email: None,
            plan: None,
            project_id: None,
            buckets,
        };
        assert_eq!(format_gemini_main_quota(&info), display.main);
        assert_eq!(format_gemini_quota_status(&info), display.status);
        assert_eq!(gemini_quota_is_ready(&info), display.ready);
        assert_eq!(format_gemini_bucket_summaries(&info), expected_summaries);
    }

    #[test]
    fn gemini_numeric_batches_preserve_full_rendering_after_abi_limit() {
        let bucket = GeminiQuotaBucket {
            remaining_amount: Some("50".to_string()),
            remaining_fraction: Some(0.5),
            reset_time: None,
            token_type: None,
            model_id: None,
        };
        let mut buckets =
            vec![bucket; prodex_mojo_core::quota::QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT + 1];
        let last = buckets.last_mut().expect("nonempty bucket list");
        last.remaining_amount = Some("0".to_string());
        last.remaining_fraction = Some(0.0);
        let info = GeminiQuotaInfo {
            email: None,
            plan: None,
            project_id: None,
            buckets,
        };
        assert_eq!(gemini_main_remaining_percent(&info), Some(0));
        assert!(!gemini_quota_is_ready(&info));
        assert_eq!(format_gemini_main_quota(&info), "gemini 0% (1025 buckets)");
    }
}
