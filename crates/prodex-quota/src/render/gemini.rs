use super::*;

#[derive(Debug, Clone, Copy)]
struct GeminiBucketRemaining {
    remaining: i64,
    total: Option<i64>,
}

fn gemini_bucket_remaining(bucket: &GeminiQuotaBucket) -> Option<GeminiBucketRemaining> {
    if let Some(raw_remaining) = bucket.remaining_amount.as_deref() {
        let remaining = raw_remaining.trim().parse::<i64>().ok()?;
        let total = bucket
            .remaining_fraction
            .filter(|fraction| *fraction > 0.0)
            .map(|fraction| (remaining as f64 / fraction).round() as i64)
            .filter(|total| *total >= remaining);
        return Some(GeminiBucketRemaining { remaining, total });
    }

    let fraction = bucket.remaining_fraction?;
    Some(GeminiBucketRemaining {
        remaining: (fraction * 100.0).round() as i64,
        total: Some(100),
    })
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

fn gemini_blocked_buckets(info: &GeminiQuotaInfo) -> Vec<String> {
    info.buckets
        .iter()
        .filter_map(|bucket| {
            let exhausted = bucket
                .remaining_fraction
                .is_some_and(|fraction| fraction <= 0.0)
                || gemini_bucket_remaining(bucket)
                    .is_some_and(|remaining| remaining.remaining <= 0);
            exhausted.then(|| format!("{} exhausted", gemini_bucket_label(bucket)))
        })
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
        format!("Blocked ({})", blocked.join(", "))
    }
}

pub fn format_gemini_bucket_summary(bucket: &GeminiQuotaBucket) -> String {
    let label = gemini_bucket_label(bucket);
    match gemini_bucket_remaining(bucket) {
        Some(GeminiBucketRemaining {
            remaining,
            total: Some(total),
        }) => format!("{label} {remaining}/{total} left"),
        Some(GeminiBucketRemaining {
            remaining,
            total: None,
        }) => format!("{label} {remaining} left"),
        None => format!("{label} quota unknown"),
    }
}

pub fn format_gemini_main_quota(info: &GeminiQuotaInfo) -> String {
    if info.buckets.is_empty() {
        return "-".to_string();
    }
    let mut parts = info
        .buckets
        .iter()
        .take(3)
        .map(format_gemini_bucket_summary)
        .collect::<Vec<_>>();
    if info.buckets.len() > parts.len() {
        parts.push(format!("+{}", info.buckets.len() - parts.len()));
    }
    parts.join(" | ")
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
