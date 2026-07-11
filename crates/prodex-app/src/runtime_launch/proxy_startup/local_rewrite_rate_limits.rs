use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use chrono::{DateTime, Utc};
use prodex_quota::{GeminiQuotaBucket, GeminiQuotaInfo};
use reqwest::header::HeaderMap;

const MAX_GEMINI_STATUS_BUCKETS: usize = 8;

pub(super) fn runtime_gemini_quota_codex_headers(
    profile_name: &str,
    profile_email: Option<&str>,
    info: &GeminiQuotaInfo,
) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    let account = profile_email
        .or(info.email.as_deref())
        .map(clean_header_value)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| clean_header_value(profile_name));

    for (index, bucket) in info
        .buckets
        .iter()
        .filter(|bucket| bucket.remaining_fraction.is_some())
        .take(MAX_GEMINI_STATUS_BUCKETS)
        .enumerate()
    {
        let Some(used_percent) = gemini_bucket_used_percent(bucket) else {
            continue;
        };
        let limit_id = if index == 0 {
            "gemini".to_string()
        } else {
            format!("gemini_{}", index + 1)
        };
        let mut label = "Gemini".to_string();
        if let Some(model) = gemini_bucket_model_label(bucket) {
            label.push(' ');
            label.push_str(&model);
        }
        if !account.is_empty() {
            label.push_str(" (");
            label.push_str(&account);
            label.push(')');
        }

        push_codex_rate_limit_header(&mut headers, &limit_id, "limit-name", label);
        push_codex_rate_limit_header(
            &mut headers,
            &limit_id,
            "primary-used-percent",
            format_percent(used_percent),
        );
        if let Some(reset_at) = bucket
            .reset_time
            .as_deref()
            .and_then(parse_rfc3339_epoch_seconds)
        {
            push_codex_rate_limit_header(
                &mut headers,
                &limit_id,
                "primary-reset-at",
                reset_at.to_string(),
            );
        }
    }

    headers
}

pub(super) fn runtime_deepseek_codex_rate_limit_headers(
    headers: &HeaderMap,
) -> Vec<(String, String)> {
    runtime_provider_codex_rate_limit_headers(RuntimeProviderBridgeKind::DeepSeek, headers)
}

pub(super) fn runtime_provider_codex_rate_limit_headers(
    provider_kind: RuntimeProviderBridgeKind,
    headers: &HeaderMap,
) -> Vec<(String, String)> {
    runtime_openai_style_codex_rate_limit_headers(
        headers,
        provider_kind.rate_limit_header_prefix(),
        provider_kind.rate_limit_header_label(),
    )
}

pub(super) fn runtime_openai_style_codex_rate_limit_headers(
    headers: &HeaderMap,
    provider_prefix: &str,
    provider_label: &str,
) -> Vec<(String, String)> {
    let mut output = Vec::new();
    push_openai_style_limit_headers(
        &mut output,
        headers,
        &format!("{provider_prefix}_requests"),
        &format!("{provider_label} requests"),
        "x-ratelimit-limit-requests",
        "x-ratelimit-remaining-requests",
        "x-ratelimit-reset-requests",
    );
    push_openai_style_limit_headers(
        &mut output,
        headers,
        &format!("{provider_prefix}_tokens"),
        &format!("{provider_label} tokens"),
        "x-ratelimit-limit-tokens",
        "x-ratelimit-remaining-tokens",
        "x-ratelimit-reset-tokens",
    );
    output
}

pub(super) fn append_text_rate_limit_headers(
    headers: &mut Vec<(String, String)>,
    extra: Vec<(String, String)>,
) {
    headers.extend(extra);
}

pub(super) fn append_binary_rate_limit_headers(
    headers: &mut Vec<(String, Vec<u8>)>,
    extra: Vec<(String, String)>,
) {
    headers.extend(
        extra
            .into_iter()
            .map(|(name, value)| (name, value.into_bytes())),
    );
}

fn gemini_bucket_used_percent(bucket: &GeminiQuotaBucket) -> Option<f64> {
    let remaining = bucket.remaining_fraction?;
    Some((100.0 - remaining * 100.0).clamp(0.0, 100.0))
}

fn gemini_bucket_model_label(bucket: &GeminiQuotaBucket) -> Option<String> {
    bucket
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.strip_prefix("models/").unwrap_or(value))
        .or_else(|| {
            bucket
                .token_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .map(clean_header_value)
        .filter(|value| !value.is_empty())
}

fn push_openai_style_limit_headers(
    output: &mut Vec<(String, String)>,
    headers: &HeaderMap,
    limit_id: &str,
    limit_name: &str,
    limit_header: &str,
    remaining_header: &str,
    reset_header: &str,
) {
    let Some(limit) = header_f64(headers, limit_header).filter(|value| *value > 0.0) else {
        return;
    };
    let Some(remaining) = header_f64(headers, remaining_header) else {
        return;
    };
    let used_percent = ((limit - remaining).max(0.0) / limit * 100.0).clamp(0.0, 100.0);
    push_codex_rate_limit_header(output, limit_id, "limit-name", limit_name.to_string());
    push_codex_rate_limit_header(
        output,
        limit_id,
        "primary-used-percent",
        format_percent(used_percent),
    );
    if let Some(reset_at) = header_value(headers, reset_header).and_then(parse_reset_epoch_seconds)
    {
        push_codex_rate_limit_header(output, limit_id, "primary-reset-at", reset_at.to_string());
    }
}

fn push_codex_rate_limit_header(
    headers: &mut Vec<(String, String)>,
    limit_id: &str,
    suffix: &str,
    value: String,
) {
    let header_limit = sanitize_limit_id(limit_id).replace('_', "-");
    headers.push((
        format!("x-{header_limit}-{suffix}"),
        clean_header_value(&value),
    ));
}

fn sanitize_limit_id(value: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if matches!(ch, '-' | '_') {
            Some('_')
        } else {
            None
        };
        let Some(next) = next else {
            continue;
        };
        if next == '_' {
            if previous_dash {
                continue;
            }
            previous_dash = true;
        } else {
            previous_dash = false;
        }
        output.push(next);
    }
    output.trim_matches('_').to_string()
}

fn format_percent(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded.fract()).abs() < f64::EPSILON {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

fn clean_header_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n'))
        .collect::<String>()
        .trim()
        .to_string()
}

fn header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    header_value(headers, name)?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn parse_reset_epoch_seconds(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(epoch) = parse_rfc3339_epoch_seconds(value) {
        return Some(epoch);
    }
    if let Ok(number) = value.parse::<i64>() {
        let now = Utc::now().timestamp();
        return Some(if number > 10_000_000_000 {
            number / 1000
        } else if number > now.saturating_sub(60) {
            number
        } else {
            now.saturating_add(number.max(0))
        });
    }
    parse_duration_seconds(value).map(|seconds| Utc::now().timestamp().saturating_add(seconds))
}

fn parse_rfc3339_epoch_seconds(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|value| value.timestamp())
}

fn parse_duration_seconds(value: &str) -> Option<i64> {
    let mut total = 0.0_f64;
    let mut number = String::new();
    let mut saw_unit = false;
    let mut chars = value.trim().chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
            continue;
        }
        if ch.is_ascii_whitespace() {
            continue;
        }
        if number.is_empty() {
            return None;
        }
        let amount = number.parse::<f64>().ok()?;
        number.clear();
        let seconds = match ch {
            'd' => amount * 86_400.0,
            'h' => amount * 3_600.0,
            'm' => {
                if chars.peek().is_some_and(|next| *next == 's') {
                    let _ = chars.next();
                    amount / 1_000.0
                } else {
                    amount * 60.0
                }
            }
            's' => amount,
            _ => return None,
        };
        total += seconds;
        saw_unit = true;
    }
    if !number.is_empty() {
        total += number.parse::<f64>().ok()?;
        saw_unit = true;
    }
    saw_unit.then(|| total.ceil().max(0.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn gemini_quota_headers_adapt_remaining_fraction_to_codex_limit() {
        let info = GeminiQuotaInfo {
            email: Some("user@example.com".to_string()),
            plan: Some("pro".to_string()),
            project_id: Some("project".to_string()),
            buckets: vec![GeminiQuotaBucket {
                remaining_amount: Some("25".to_string()),
                remaining_fraction: Some(0.25),
                reset_time: Some("2026-05-31T12:00:00Z".to_string()),
                token_type: None,
                model_id: Some("models/gemini-2.5-pro".to_string()),
            }],
        };

        let headers = runtime_gemini_quota_codex_headers("gemini-main", None, &info);

        assert!(headers.contains(&(
            "x-gemini-primary-used-percent".to_string(),
            "75".to_string()
        )));
        assert!(headers.contains(&(
            "x-gemini-primary-reset-at".to_string(),
            "1780228800".to_string()
        )));
        assert!(headers.contains(&(
            "x-gemini-limit-name".to_string(),
            "Gemini gemini-2.5-pro (user@example.com)".to_string()
        )));
    }

    #[test]
    fn deepseek_headers_adapt_openai_compatible_limits_when_present() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("40"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("60s"),
        );

        let adapted = runtime_deepseek_codex_rate_limit_headers(&headers);

        assert!(adapted.contains(&(
            "x-deepseek-requests-primary-used-percent".to_string(),
            "60".to_string()
        )));
        assert!(
            adapted
                .iter()
                .any(|(name, _)| { name == "x-deepseek-requests-primary-reset-at" })
        );
    }

    #[test]
    fn provider_headers_use_bridge_kind_metadata() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("75"),
        );

        let adapted =
            runtime_provider_codex_rate_limit_headers(RuntimeProviderBridgeKind::Gemini, &headers);

        assert!(adapted.contains(&(
            "x-gemini-requests-primary-used-percent".to_string(),
            "25".to_string()
        )));
        assert!(adapted.contains(&(
            "x-gemini-requests-limit-name".to_string(),
            "Google Gemini requests".to_string()
        )));
    }
}
