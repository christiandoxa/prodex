use super::*;

#[derive(Debug, Clone, Copy)]
struct GeminiBucketNumeric {
    remaining: Option<i64>,
    total: Option<i64>,
    remaining_percent: Option<i64>,
    exhausted: bool,
}

fn gemini_numeric_input(
    bucket: &GeminiQuotaBucket,
) -> prodex_mojo_core::quota::GeminiBucketNumericInput {
    let remaining_amount = match bucket.remaining_amount.as_deref() {
        None => prodex_mojo_core::quota::GeminiRemainingAmount::Absent,
        Some(raw_remaining) => raw_remaining
            .trim()
            .parse::<i64>()
            .map(prodex_mojo_core::quota::GeminiRemainingAmount::Parsed)
            .unwrap_or(prodex_mojo_core::quota::GeminiRemainingAmount::Invalid),
    };
    prodex_mojo_core::quota::GeminiBucketNumericInput {
        remaining_amount,
        remaining_fraction: bucket.remaining_fraction,
    }
}

fn gemini_numeric_batch(buckets: &[GeminiQuotaBucket]) -> Vec<GeminiBucketNumeric> {
    // ponytail: 1,024-row ABI batches; raise only with a versioned ABI/capacity review.
    buckets
        .chunks(prodex_mojo_core::quota::QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT)
        .flat_map(|batch| {
            let inputs = batch.iter().map(gemini_numeric_input).collect::<Vec<_>>();
            crate::mojo::gemini_bucket_numeric_batch(&inputs)
                .unwrap_or_else(|error| panic!("Mojo Gemini quota numeric batch failed: {error:?}"))
                .into_iter()
                .map(|output| GeminiBucketNumeric {
                    remaining: output.remaining,
                    total: output.total,
                    remaining_percent: output.remaining_percent,
                    exhausted: output.exhausted,
                })
        })
        .collect()
}

fn gemini_bucket_label(bucket: &GeminiQuotaBucket) -> String {
    bucket
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.strip_prefix("models/").unwrap_or(value).to_string())
        .or_else(|| {
            bucket
                .token_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
        })
        .unwrap_or_else(|| "gemini".to_string())
}

pub(super) fn gemini_main_remaining_percent(info: &GeminiQuotaInfo) -> Option<i64> {
    gemini_numeric_batch(&info.buckets)
        .iter()
        .filter_map(|numeric| numeric.remaining_percent)
        .min()
}

fn gemini_blocked_buckets(info: &GeminiQuotaInfo) -> Vec<String> {
    let numeric = gemini_numeric_batch(&info.buckets);
    info.buckets
        .iter()
        .zip(numeric)
        .filter(|(_, numeric)| numeric.exhausted)
        .map(|(bucket, _)| format!("{} exhausted", gemini_bucket_label(bucket)))
        .collect()
}

pub fn gemini_quota_is_ready(info: &GeminiQuotaInfo) -> bool {
    !info.buckets.is_empty() && gemini_blocked_buckets(info).is_empty()
}

pub fn format_gemini_quota_status(info: &GeminiQuotaInfo) -> String {
    if info.buckets.is_empty() {
        return "Unknown".to_string();
    }
    let blocked = gemini_blocked_buckets(info);
    if blocked.is_empty() {
        "Ready".to_string()
    } else {
        "Blocked".to_string()
    }
}

pub(super) fn format_gemini_bucket_summaries(info: &GeminiQuotaInfo) -> Vec<String> {
    gemini_numeric_batch(&info.buckets)
        .into_iter()
        .zip(&info.buckets)
        .map(|(numeric, bucket)| {
            let label = gemini_bucket_label(bucket);
            format_gemini_bucket_summary_with_numeric(&label, numeric)
        })
        .collect()
}

fn format_gemini_bucket_summary_with_numeric(label: &str, numeric: GeminiBucketNumeric) -> String {
    match numeric.remaining {
        Some(remaining) => match numeric.total {
            Some(total) => format!("{label} {remaining}/{total}"),
            None => format!("{label} {remaining}"),
        },
        None => format!("{label} quota unknown"),
    }
}

pub fn format_gemini_main_quota(info: &GeminiQuotaInfo) -> String {
    if info.buckets.is_empty() {
        return "-".to_string();
    }
    let numeric = gemini_numeric_batch(&info.buckets);
    if let Some(percent) = numeric
        .iter()
        .filter_map(|numeric| numeric.remaining_percent)
        .min()
    {
        let bucket_count = info.buckets.len();
        return if bucket_count == 1 {
            format!("gemini {percent}%")
        } else {
            format!("gemini {percent}% ({bucket_count} buckets)")
        };
    }

    let known_amounts = numeric
        .iter()
        .filter_map(|numeric| numeric.remaining)
        .collect::<Vec<_>>();
    if known_amounts.is_empty() {
        "gemini quota unknown".to_string()
    } else {
        format!(
            "gemini {}",
            known_amounts.iter().copied().min().unwrap_or(0)
        )
    }
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
