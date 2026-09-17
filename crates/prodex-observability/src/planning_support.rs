use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

pub(crate) fn label_key(index: usize, rust_key: &'static str) -> &'static str {
    #[cfg(feature = "mojo")]
    {
        let _ = rust_key;
        crate::mojo::label_key(index)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let _ = index;
        rust_key
    }
}

pub(crate) fn metric_name(plan: usize, slot: usize, rust_name: &'static str) -> &'static str {
    #[cfg(feature = "mojo")]
    {
        let _ = rust_name;
        crate::mojo::metric_name(plan, slot)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let _ = (plan, slot);
        rust_name
    }
}

pub(crate) fn validated_metric_label(
    key: &'static str,
    value: impl Into<String>,
) -> Result<TelemetryAttribute, TelemetryAttributeError> {
    let label = TelemetryAttribute::metric_label(key, value.into());
    label.as_metric_label()?;
    Ok(label)
}

#[cfg(feature = "mojo")]
pub(crate) fn planned_metric_label(
    plan: usize,
    slot: usize,
    value: i64,
) -> Result<TelemetryAttribute, TelemetryAttributeError> {
    let spec = prodex_mojo_core::observability::plan_label_spec(plan as i64, slot as i64)
        .expect("Mojo observability plan-label metadata returned invalid output");
    let key = crate::mojo::label_key(
        usize::try_from(spec.key).expect("Mojo observability label-key index is non-negative"),
    );
    let value = prodex_mojo_core::observability::label(spec.kind, value)
        .expect("Mojo observability plan-label value returned invalid output");
    validated_metric_label(key, value)
}
