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
