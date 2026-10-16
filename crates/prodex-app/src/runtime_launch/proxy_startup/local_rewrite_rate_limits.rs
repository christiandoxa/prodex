use crate::runtime_launch::proxy_startup::provider_bridge::RuntimeProviderBridgeKind;
use chrono::{DateTime, Utc};
use prodex_mojo_core::provider_constraints::{
    provider_quota_clean_header, provider_quota_duration_reset_epoch,
    provider_quota_gemini_used_percent, provider_quota_limit_id, provider_quota_model_label,
    provider_quota_numeric_reset_epoch, provider_quota_openai_used_percent,
};
use prodex_quota::GeminiQuotaInfo;
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
        .map(|value| {
            provider_quota_clean_header(value)
                .expect("compiled Mojo quota account label returned invalid output")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            provider_quota_clean_header(profile_name)
                .expect("compiled Mojo quota profile label returned invalid output")
        });

    for (index, bucket) in info
        .buckets
        .iter()
        .filter(|bucket| bucket.remaining_fraction.is_some())
        .take(MAX_GEMINI_STATUS_BUCKETS)
        .enumerate()
    {
        let Some(used_percent) = provider_quota_gemini_used_percent(bucket.remaining_fraction)
            .expect("compiled Mojo Gemini quota percentage returned invalid output")
        else {
            continue;
        };
        let limit_id = if index == 0 {
            "gemini".to_string()
        } else {
            format!("gemini_{}", index + 1)
        };
        let mut label = "Gemini".to_string();
        if let Some(model) =
            provider_quota_model_label(bucket.model_id.as_deref(), bucket.token_type.as_deref())
                .expect("compiled Mojo quota model label returned invalid output")
        {
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
            used_percent,
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

fn push_openai_style_limit_headers(
    output: &mut Vec<(String, String)>,
    headers: &HeaderMap,
    limit_id: &str,
    limit_name: &str,
    limit_header: &str,
    remaining_header: &str,
    reset_header: &str,
) {
    let Some(used_percent) = provider_quota_openai_used_percent(
        header_f64(headers, limit_header),
        header_f64(headers, remaining_header),
    )
    .expect("compiled Mojo OpenAI-compatible quota percentage returned invalid output") else {
        return;
    };
    push_codex_rate_limit_header(output, limit_id, "limit-name", limit_name.to_string());
    push_codex_rate_limit_header(output, limit_id, "primary-used-percent", used_percent);
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
    let header_limit = provider_quota_limit_id(limit_id)
        .expect("compiled Mojo Codex quota header ID returned invalid output");
    headers.push((
        format!("x-{header_limit}-{suffix}"),
        provider_quota_clean_header(&value)
            .expect("compiled Mojo Codex quota header value returned invalid output"),
    ));
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
        return Some(
            provider_quota_numeric_reset_epoch(number, now)
                .expect("compiled Mojo quota numeric reset decision returned invalid output"),
        );
    }
    parse_duration_seconds(value).map(|seconds| {
        provider_quota_duration_reset_epoch(seconds, Utc::now().timestamp())
            .expect("compiled Mojo quota duration reset decision returned invalid output")
    })
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
    use prodex_quota::GeminiQuotaBucket;
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
    fn quota_header_model_source_and_value_cleaning_are_mojo_owned() {
        let buckets = vec![
            GeminiQuotaBucket {
                remaining_amount: None,
                remaining_fraction: Some(0.5),
                reset_time: None,
                token_type: Some("ignored".to_string()),
                model_id: Some("  models/東京🌐  ".to_string()),
            },
            GeminiQuotaBucket {
                remaining_amount: None,
                remaining_fraction: Some(0.0),
                reset_time: None,
                token_type: Some("  tok\r\n en  ".to_string()),
                model_id: Some(" \u{2003} ".to_string()),
            },
            GeminiQuotaBucket {
                remaining_amount: None,
                remaining_fraction: Some(1.0),
                reset_time: None,
                token_type: Some("ignored".to_string()),
                model_id: Some("models/".to_string()),
            },
        ];
        let info = GeminiQuotaInfo {
            email: Some(" \r\n\u{2003} ".to_string()),
            plan: None,
            project_id: None,
            buckets,
        };
        let headers =
            runtime_gemini_quota_codex_headers(" \u{2003}profile\r\n-main \u{3000}", None, &info);
        assert!(headers.contains(&(
            "x-gemini-limit-name".to_string(),
            "Gemini 東京🌐 (profile-main)".to_string(),
        )));
        assert!(headers.contains(&(
            "x-gemini-2-limit-name".to_string(),
            "Gemini tok en (profile-main)".to_string(),
        )));
        assert!(headers.contains(&(
            "x-gemini-3-limit-name".to_string(),
            "Gemini (profile-main)".to_string(),
        )));
        assert!(
            headers
                .iter()
                .all(|(_, value)| !value.contains('\r') && !value.contains('\n'))
        );
    }

    #[test]
    fn quota_header_cleaning_is_used_for_openai_style_provider_labels() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("0"),
        );
        let projected = runtime_openai_style_codex_rate_limit_headers(
            &headers,
            "local",
            " \r\n\u{2003}Local \n Provider\u{3000}",
        );
        assert!(
            projected.contains(&(
                "x-local-requests-limit-name".to_string(),
                "Local  Provider\u{3000} requests".to_string(),
            )),
            "{projected:?}"
        );
        assert!(projected.contains(&(
            "x-local-requests-primary-used-percent".to_string(),
            "100".to_string(),
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
    fn quota_reset_headers_observe_compiled_mojo_absolute_and_relative_epochs() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("1"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("1234567890123"),
        );
        let mapped = runtime_openai_style_codex_rate_limit_headers(&headers, "test", "Test");
        assert!(mapped.contains(&(
            "x-test-requests-primary-reset-at".to_string(),
            "1234567890".to_string()
        )));

        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("60s"),
        );
        let before = Utc::now().timestamp();
        let mapped = runtime_openai_style_codex_rate_limit_headers(&headers, "test", "Test");
        let after = Utc::now().timestamp();
        let timestamp: i64 = mapped
            .iter()
            .find(|(name, _)| name == "x-test-requests-primary-reset-at")
            .expect("Mojo-backed duration reset is present")
            .1
            .parse()
            .unwrap();
        assert!(timestamp >= before.saturating_add(60));
        assert!(timestamp <= after.saturating_add(60));
    }

    #[test]
    fn quota_header_limit_id_unicode_projection_is_compiled_mojo_owned() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("25"),
        );
        let result =
            runtime_openai_style_codex_rate_limit_headers(&headers, " K___İ _API__ 🧭", "Provider");
        assert!(
            result.contains(&(
                "x-k-i-api-requests-limit-name".to_string(),
                "Provider requests".to_string(),
            )),
            "{result:?}"
        );
        assert!(result.contains(&(
            "x-k-i-api-requests-primary-used-percent".to_string(),
            "75".to_string(),
        )));
    }

    #[test]
    fn quota_used_percent_rounding_and_saturation_are_mojo_owned() {
        let info = GeminiQuotaInfo {
            email: None,
            plan: None,
            project_id: None,
            buckets: vec![
                GeminiQuotaBucket {
                    remaining_amount: None,
                    remaining_fraction: Some(0.333),
                    reset_time: None,
                    token_type: Some("first".to_string()),
                    model_id: None,
                },
                GeminiQuotaBucket {
                    remaining_amount: None,
                    remaining_fraction: Some(1.5),
                    reset_time: None,
                    token_type: Some("second".to_string()),
                    model_id: None,
                },
            ],
        };
        let projected = runtime_gemini_quota_codex_headers("profile", None, &info);
        assert!(projected.contains(&(
            "x-gemini-primary-used-percent".to_string(),
            "66.7".to_string(),
        )));
        assert!(projected.contains(&(
            "x-gemini-2-primary-used-percent".to_string(),
            "0".to_string(),
        )));

        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-limit-tokens", HeaderValue::from_static("3"));
        headers.insert(
            "x-ratelimit-remaining-tokens",
            HeaderValue::from_static("1"),
        );
        let projected = runtime_openai_style_codex_rate_limit_headers(&headers, "edge", "Edge");
        assert!(projected.contains(&(
            "x-edge-tokens-primary-used-percent".to_string(),
            "66.7".to_string(),
        )));
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
