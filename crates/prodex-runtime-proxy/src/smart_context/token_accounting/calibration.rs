#[cfg(any(not(feature = "mojo"), test))]
use super::estimation::smart_context_estimate_tokens_from_body_bytes_rust;
use super::*;
use crate::RuntimeTokenUsage;

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_NUMERATOR: u64 = 9;
#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR: u64 = 8;
#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS: u64 = 64;
#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT: usize = 4;
#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) const SMART_CONTEXT_ADAPTIVE_ESTIMATE_MAX_INFLATION: u64 = 2;

#[cfg(feature = "mojo")]
pub(in crate::smart_context) fn smart_context_observed_calibrated_request_estimate(
    body_bytes: usize,
    baseline_estimate: u64,
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> u64 {
    #[cfg(feature = "mojo")]
    {
        let bucket = calibration_bucket_key.map(|bucket| {
            prodex_mojo_core::runtime::SmartContextCalibrationBucket {
                route: bucket.route.as_deref(),
                model: bucket.model.as_deref(),
                profile: bucket.profile.as_deref(),
                transport: bucket.transport.as_deref(),
            }
        });
        let samples = calibration_samples
            .iter()
            .map(
                |sample| prodex_mojo_core::runtime::SmartContextCalibrationSample {
                    bucket: sample.bucket_key.as_ref().map(|bucket| {
                        prodex_mojo_core::runtime::SmartContextCalibrationBucket {
                            route: bucket.route.as_deref(),
                            model: bucket.model.as_deref(),
                            profile: bucket.profile.as_deref(),
                            transport: bucket.transport.as_deref(),
                        }
                    }),
                    input_tokens: sample.usage.input_tokens,
                    cached_input_tokens: sample.usage.cached_input_tokens,
                },
            )
            .collect::<Vec<_>>();
        let usage = observed_usage
            .iter()
            .map(
                |usage| prodex_mojo_core::runtime::SmartContextCalibrationUsage {
                    input_tokens: usage.input_tokens,
                    cached_input_tokens: usage.cached_input_tokens,
                },
            )
            .collect::<Vec<_>>();
        let observed_accounted_input =
            prodex_mojo_core::runtime::smart_context_calibration_observed_input(
                bucket, &samples, &usage,
            )
            .expect("Mojo Smart Context calibration selection returned invalid output");
        prodex_mojo_core::runtime::smart_context_calibrated_estimate_batch(&[
            prodex_mojo_core::runtime::SmartContextCalibratedEstimateInput {
                body_bytes: u64::try_from(body_bytes).expect("request body length fits u64"),
                baseline_estimate,
                observed_accounted_input,
            },
        ])
        .expect("Mojo Smart Context calibration returned invalid output")[0]
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_observed_calibrated_request_estimate_rust(
        body_bytes,
        baseline_estimate,
        observed_usage,
        calibration_bucket_key,
        calibration_samples,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn smart_context_observed_calibrated_request_estimate_rust(
    body_bytes: usize,
    baseline_estimate: u64,
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> u64 {
    if baseline_estimate == 0 {
        return 0;
    }
    let Some(recent_accounted_input) = smart_context_recent_accounted_input_calibration(
        observed_usage,
        calibration_bucket_key,
        calibration_samples,
    ) else {
        return baseline_estimate;
    };
    let raw_floor = smart_context_estimate_tokens_from_body_bytes_rust(body_bytes)
        .saturating_add(1)
        .saturating_div(2)
        .max(1);
    let raw_floor_with_margin =
        raw_floor.saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS);
    let observed_with_margin = recent_accounted_input
        .saturating_mul(SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_NUMERATOR)
        .saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR - 1)
        / SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR;
    let observed_with_margin =
        observed_with_margin.saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS);
    let calibrated = observed_with_margin.max(raw_floor_with_margin);
    if calibrated > baseline_estimate.saturating_mul(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MAX_INFLATION)
    {
        baseline_estimate
    } else {
        calibrated
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_recent_accounted_input_calibration(
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> Option<u64> {
    if !calibration_samples.is_empty() {
        return smart_context_recent_accounted_input_calibration_for_bucket(
            calibration_bucket_key,
            calibration_samples,
        );
    }

    observed_usage
        .iter()
        .rev()
        .filter_map(|usage| smart_context_accounted_input_tokens(*usage))
        .take(SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT)
        .max()
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_recent_accounted_input_calibration_for_bucket(
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> Option<u64> {
    if let Some(calibration) = smart_context_recent_accounted_input_calibration_matching(
        calibration_samples,
        |sample_bucket_key| sample_bucket_key == calibration_bucket_key,
    ) {
        return Some(calibration);
    }

    let calibration_bucket_key = calibration_bucket_key?;

    for tier in [
        SmartContextTokenCalibrationFallbackTier::Model,
        SmartContextTokenCalibrationFallbackTier::ProfileRoute,
        SmartContextTokenCalibrationFallbackTier::RouteTransportGlobal,
    ] {
        if let Some(calibration) = smart_context_recent_accounted_input_calibration_matching(
            calibration_samples,
            |sample_bucket_key| {
                smart_context_token_calibration_bucket_fallback_matches(
                    calibration_bucket_key,
                    sample_bucket_key,
                    tier,
                )
            },
        ) {
            return Some(calibration);
        }
    }

    None
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_recent_accounted_input_calibration_matching(
    calibration_samples: &[SmartContextTokenCalibrationSample],
    mut bucket_matches: impl FnMut(Option<&SmartContextTokenCalibrationBucketKey>) -> bool,
) -> Option<u64> {
    calibration_samples
        .iter()
        .rev()
        .filter(|sample| bucket_matches(sample.bucket_key.as_ref()))
        .filter_map(|sample| smart_context_accounted_input_tokens(sample.usage))
        .take(SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT)
        .max()
}

#[cfg(any(not(feature = "mojo"), test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::smart_context) enum SmartContextTokenCalibrationFallbackTier {
    Model,
    ProfileRoute,
    RouteTransportGlobal,
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_bucket_fallback_matches(
    target: &SmartContextTokenCalibrationBucketKey,
    sample: Option<&SmartContextTokenCalibrationBucketKey>,
    tier: SmartContextTokenCalibrationFallbackTier,
) -> bool {
    match tier {
        SmartContextTokenCalibrationFallbackTier::Model => sample.is_some_and(|sample| {
            smart_context_token_calibration_model_matches(&target.model, &sample.model)
        }),
        SmartContextTokenCalibrationFallbackTier::ProfileRoute => sample.is_some_and(|sample| {
            smart_context_token_calibration_field_matches(&target.profile, &sample.profile)
                && smart_context_token_calibration_field_matches(&target.route, &sample.route)
        }),
        SmartContextTokenCalibrationFallbackTier::RouteTransportGlobal => sample
            .map(|sample| {
                (smart_context_token_calibration_field_matches(&target.route, &sample.route)
                    && smart_context_token_calibration_field_matches(
                        &target.transport,
                        &sample.transport,
                    ))
                    || smart_context_token_calibration_sample_is_global_compatible(target, sample)
            })
            .unwrap_or(true),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_field_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    target.as_deref().is_some_and(non_empty) && target == sample
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_model_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    let Some(target) = smart_context_token_calibration_normalized_model(target.as_deref()) else {
        return false;
    };
    let Some(sample) = smart_context_token_calibration_normalized_model(sample.as_deref()) else {
        return false;
    };
    target == sample
        || smart_context_token_calibration_model_family(&target)
            == smart_context_token_calibration_model_family(&sample)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_sample_is_global_compatible(
    target: &SmartContextTokenCalibrationBucketKey,
    sample: &SmartContextTokenCalibrationBucketKey,
) -> bool {
    smart_context_token_calibration_optional_field_compatible(&target.route, &sample.route)
        && smart_context_token_calibration_optional_model_compatible(&target.model, &sample.model)
        && smart_context_token_calibration_optional_field_compatible(
            &target.profile,
            &sample.profile,
        )
        && smart_context_token_calibration_optional_field_compatible(
            &target.transport,
            &sample.transport,
        )
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_optional_field_compatible(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    sample
        .as_deref()
        .is_none_or(|sample| target.as_deref() == Some(sample))
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_optional_model_compatible(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    sample
        .as_ref()
        .is_none_or(|_| smart_context_token_calibration_model_matches(target, sample))
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_normalized_model(
    value: Option<&str>,
) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let mut normalized = String::with_capacity(value.len());
    let mut previous_separator = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        let ch = match ch {
            '_' | ' ' => '-',
            ch => ch,
        };
        if ch == '-' {
            if !previous_separator {
                normalized.push(ch);
                previous_separator = true;
            }
        } else {
            normalized.push(ch);
            previous_separator = false;
        }
    }
    let normalized = normalized.trim_matches('-');
    (!normalized.is_empty()).then(|| normalized.to_string())
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_calibration_model_family(model: &str) -> &str {
    let model = model
        .strip_suffix("-latest")
        .or_else(|| model.strip_suffix("-preview"))
        .unwrap_or(model);

    if let Some(family) = model
        .strip_suffix("-mini")
        .or_else(|| model.strip_suffix("-nano"))
        .or_else(|| model.strip_suffix("-codex"))
    {
        return family;
    }

    if model.len() >= 11 {
        let suffix_start = model.len() - 11;
        let suffix = &model[suffix_start..];
        if suffix.as_bytes()[0] == b'-'
            && suffix.as_bytes()[1..5].iter().all(u8::is_ascii_digit)
            && suffix.as_bytes()[5] == b'-'
            && suffix.as_bytes()[6..8].iter().all(u8::is_ascii_digit)
            && suffix.as_bytes()[8] == b'-'
            && suffix.as_bytes()[9..11].iter().all(u8::is_ascii_digit)
        {
            return &model[..suffix_start];
        }
    }

    model
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_accounted_input_tokens(
    usage: RuntimeTokenUsage,
) -> Option<u64> {
    let accounted = if usage.input_tokens == 0 {
        usage.cached_input_tokens
    } else {
        usage.input_tokens
    };
    (accounted > 0).then_some(accounted)
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn calibrated_estimate_batch_matches_rust_oracle() {
        let cases = [
            (0, 0, Vec::new(), Vec::new()),
            (
                4_000,
                1_000,
                vec![RuntimeTokenUsage {
                    input_tokens: 600,
                    ..Default::default()
                }],
                Vec::new(),
            ),
            (
                4_000,
                1_000,
                vec![RuntimeTokenUsage {
                    input_tokens: 2_000,
                    ..Default::default()
                }],
                Vec::new(),
            ),
            (
                8_000,
                2_000,
                Vec::new(),
                vec![SmartContextTokenCalibrationSample {
                    bucket_key: None,
                    usage: RuntimeTokenUsage {
                        input_tokens: 900,
                        ..Default::default()
                    },
                }],
            ),
        ];
        for (body_bytes, baseline, observed_usage, calibration_samples) in cases {
            let bucket = (!calibration_samples.is_empty())
                .then_some(SmartContextTokenCalibrationBucketKey::default());
            let expected = smart_context_observed_calibrated_request_estimate_rust(
                body_bytes,
                baseline,
                &observed_usage,
                bucket.as_ref(),
                &calibration_samples,
            );
            let observed = smart_context_recent_accounted_input_calibration(
                &observed_usage,
                bucket.as_ref(),
                &calibration_samples,
            );
            let actual = prodex_mojo_core::runtime::smart_context_calibrated_estimate_batch(&[
                prodex_mojo_core::runtime::SmartContextCalibratedEstimateInput {
                    body_bytes: body_bytes as u64,
                    baseline_estimate: baseline,
                    observed_accounted_input: observed,
                },
            ])
            .expect("calibration batch should accept the normalized input")[0];
            assert_eq!(
                actual, expected,
                "body_bytes={body_bytes} baseline={baseline}"
            );
        }
    }
}
