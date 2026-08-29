const SMART_CONTEXT_CALIBRATION_MAX_COUNT: usize = 256;
const SMART_CONTEXT_CALIBRATION_MAX_BYTES: usize = 4_096;

#[repr(C)]
#[derive(Clone, Copy)]
struct RuntimeStringView {
    ptr: u64,
    len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextCalibrationBucket<'a> {
    pub route: Option<&'a str>,
    pub model: Option<&'a str>,
    pub profile: Option<&'a str>,
    pub transport: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextCalibrationSample<'a> {
    pub bucket: Option<SmartContextCalibrationBucket<'a>>,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextCalibrationUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
}

pub fn smart_context_calibration_models_match(
    target: &str,
    sample: &str,
) -> Result<bool, crate::MojoError> {
    let target_view = string_view(Some(target));
    let sample_view = string_view(Some(sample));
    let status = unsafe {
        prodex_smart_context_calibration_models_match_v1(
            &target_view as *const RuntimeStringView as usize as u64,
            &sample_view as *const RuntimeStringView as usize as u64,
        )
    };
    match status {
        0 => Ok(false),
        1 => Ok(true),
        2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

unsafe extern "C" {
    fn prodex_smart_context_calibration_models_match_v1(
        target_view_address: u64,
        sample_view_address: u64,
    ) -> i64;
    fn prodex_smart_context_calibration_observed_input_v1(
        target_views: *const RuntimeStringView,
        target_mask: i64,
        sample_views: *const RuntimeStringView,
        sample_masks: *const i64,
        sample_input_tokens: *const u64,
        sample_cached_input_tokens: *const u64,
        sample_count: i64,
        observed_input_tokens: *const u64,
        observed_cached_input_tokens: *const u64,
        observed_count: i64,
        observed_accounted_input: *mut u64,
        observed_present: *mut i64,
    ) -> i64;
}

pub fn smart_context_calibration_observed_input(
    target: Option<SmartContextCalibrationBucket<'_>>,
    samples: &[SmartContextCalibrationSample<'_>],
    observed_usage: &[SmartContextCalibrationUsage],
) -> Result<Option<u64>, crate::MojoError> {
    if samples.len() > SMART_CONTEXT_CALIBRATION_MAX_COUNT
        || observed_usage.len() > SMART_CONTEXT_CALIBRATION_MAX_COUNT
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let target_views = target
        .map(|bucket| bucket_views(Some(bucket)))
        .unwrap_or([RuntimeStringView { ptr: 0, len: 0 }; 4]);
    let target_mask = target.map(bucket_mask).unwrap_or(0);
    let mut sample_views = Vec::with_capacity(samples.len() * 4);
    let mut sample_masks = Vec::with_capacity(samples.len());
    let mut sample_input_tokens = Vec::with_capacity(samples.len());
    let mut sample_cached_input_tokens = Vec::with_capacity(samples.len());
    for sample in samples {
        if sample.bucket.is_none() {
            sample_views.extend([RuntimeStringView { ptr: 0, len: 0 }; 4]);
        } else {
            sample_views.extend(bucket_views(sample.bucket));
        }
        sample_masks.push(sample.bucket.map(bucket_mask).unwrap_or(0));
        sample_input_tokens.push(sample.input_tokens);
        sample_cached_input_tokens.push(sample.cached_input_tokens);
    }
    let observed_input_tokens = observed_usage
        .iter()
        .map(|usage| usage.input_tokens)
        .collect::<Vec<_>>();
    let observed_cached_input_tokens = observed_usage
        .iter()
        .map(|usage| usage.cached_input_tokens)
        .collect::<Vec<_>>();
    if target_views
        .iter()
        .chain(&sample_views)
        .any(|view| view.len as usize > SMART_CONTEXT_CALIBRATION_MAX_BYTES)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut observed_accounted_input = 0_u64;
    let mut observed_present = 0_i64;
    let status = unsafe {
        prodex_smart_context_calibration_observed_input_v1(
            target_views.as_ptr(),
            target_mask,
            sample_views.as_ptr(),
            sample_masks.as_ptr(),
            sample_input_tokens.as_ptr(),
            sample_cached_input_tokens.as_ptr(),
            i64::try_from(samples.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            observed_input_tokens.as_ptr(),
            observed_cached_input_tokens.as_ptr(),
            i64::try_from(observed_usage.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut observed_accounted_input,
            &mut observed_present,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    match observed_present {
        0 => Ok(None),
        1 => Ok(Some(observed_accounted_input)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

fn bucket_views(bucket: Option<SmartContextCalibrationBucket<'_>>) -> [RuntimeStringView; 4] {
    let Some(bucket) = bucket else {
        return [RuntimeStringView { ptr: 0, len: 0 }; 4];
    };
    [
        string_view(bucket.route),
        string_view(bucket.model),
        string_view(bucket.profile),
        string_view(bucket.transport),
    ]
}

fn bucket_mask(bucket: SmartContextCalibrationBucket<'_>) -> i64 {
    [bucket.route, bucket.model, bucket.profile, bucket.transport]
        .into_iter()
        .enumerate()
        .fold(16, |mask, (index, value)| {
            mask | i64::from(value.is_some()) << index
        })
}

fn string_view(value: Option<&str>) -> RuntimeStringView {
    value
        .map(|value| RuntimeStringView {
            ptr: value.as_ptr() as usize as u64,
            len: value.len() as u64,
        })
        .unwrap_or(RuntimeStringView { ptr: 0, len: 0 })
}

const _: () = {
    assert!(std::mem::size_of::<RuntimeStringView>() == 16);
    assert!(std::mem::align_of::<RuntimeStringView>() == 8);
    assert!(std::mem::offset_of!(RuntimeStringView, ptr) == 0);
    assert!(std::mem::offset_of!(RuntimeStringView, len) == 8);
};
