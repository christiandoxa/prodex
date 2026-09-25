use super::*;
use crate::RuntimeTokenUsage;

pub(in crate::smart_context) fn smart_context_observed_calibrated_request_estimate(
    body_bytes: usize,
    baseline_estimate: u64,
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> u64 {
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
