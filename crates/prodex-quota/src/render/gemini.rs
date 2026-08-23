use super::*;

#[derive(Debug, Clone, Copy)]
struct GeminiBucketNumeric {
    remaining: Option<i64>,
    total: Option<i64>,
    remaining_percent: Option<i64>,
    exhausted: bool,
}

#[cfg(feature = "mojo")]
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
    #[cfg(feature = "mojo")]
    {
        let inputs = buckets.iter().map(gemini_numeric_input).collect::<Vec<_>>();
        crate::mojo::gemini_bucket_numeric_batch(&inputs)
            .unwrap_or_else(|error| panic!("Mojo Gemini quota numeric batch failed: {error:?}"))
            .into_iter()
            .map(|output| GeminiBucketNumeric {
                remaining: output.remaining,
                total: output.total,
                remaining_percent: output.remaining_percent,
                exhausted: output.exhausted,
            })
            .collect()
    }

    #[cfg(not(feature = "mojo"))]
    buckets.iter().map(gemini_bucket_numeric_rust).collect()
}

fn gemini_bucket_numeric(bucket: &GeminiQuotaBucket) -> GeminiBucketNumeric {
    gemini_numeric_batch(std::slice::from_ref(bucket))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("Mojo Gemini quota numeric batch returned no bucket"))
}

#[cfg(not(feature = "mojo"))]
fn gemini_bucket_numeric_rust(bucket: &GeminiQuotaBucket) -> GeminiBucketNumeric {
    let (remaining, total) = match bucket.remaining_amount.as_deref() {
        Some(raw) => match raw.trim().parse::<i64>() {
            Ok(remaining) => (
                Some(remaining),
                bucket
                    .remaining_fraction
                    .filter(|fraction| *fraction > 0.0)
                    .map(|fraction| super::round_quota_float(remaining as f64 / fraction))
                    .filter(|total| *total >= remaining),
            ),
            Err(_) => (None, None),
        },
        None => match bucket.remaining_fraction {
            Some(fraction) => (Some(super::round_quota_float(fraction * 100.0)), Some(100)),
            None => (None, None),
        },
    };
    let remaining_percent = bucket
        .remaining_fraction
        .map(|fraction| super::round_quota_float(fraction * 100.0))
        .or_else(|| {
            let (Some(remaining), Some(total)) = (remaining, total) else {
                return None;
            };
            (total > 0).then(|| super::round_quota_float(remaining as f64 / total as f64 * 100.0))
        });

    GeminiBucketNumeric {
        remaining,
        total,
        remaining_percent,
        exhausted: bucket
            .remaining_fraction
            .is_some_and(|fraction| fraction <= 0.0)
            || remaining.is_some_and(|remaining| remaining <= 0),
    }
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

pub fn format_gemini_bucket_summary(bucket: &GeminiQuotaBucket) -> String {
    let label = gemini_bucket_label(bucket);
    let numeric = gemini_bucket_numeric(bucket);
    format_gemini_bucket_summary_with_numeric(&label, numeric)
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
