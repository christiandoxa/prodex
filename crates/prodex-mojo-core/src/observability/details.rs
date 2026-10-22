use super::{
    OBSERVABILITY_LABEL_ABI_VERSION, OperationalDetailFormat, stream::TextView,
    text::render_measured_text,
};
use crate::MojoError;

/// One compiled metadata descriptor and its host-acquired display text.
/// `prepared` must already be redacted (plain/percent) or URL-parsed (endpoint).
pub struct OperationalDetailText<'a> {
    pub label: &'a str,
    pub format: OperationalDetailFormat,
    pub original: &'a str,
    pub prepared: &'a str,
}

#[repr(C)]
struct DetailView {
    label: TextView,
    original: TextView,
    prepared: TextView,
    format: i64,
}

const _: () = assert!(std::mem::size_of::<DetailView>() == 56);

unsafe extern "C" {
    fn prodex_mojo_operational_log_details_v1(
        abi_version: i64,
        title_address: u64,
        title_length: i64,
        fields_address: u64,
        field_count: i64,
        measuring: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Render ordered details through the required compiled inclusion/format policy.
pub fn render_operational_details(
    title: &str,
    details: &[OperationalDetailText<'_>],
) -> Result<String, MojoError> {
    if details.len() > 64 {
        return Err(MojoError::InvalidInput);
    }
    let fields: Vec<_> = details
        .iter()
        .map(|field| DetailView {
            label: field.label.into(),
            original: field.original.into(),
            prepared: field.prepared.into(),
            format: match field.format {
                OperationalDetailFormat::Plain => 0,
                OperationalDetailFormat::Percent => 1,
                OperationalDetailFormat::Endpoint => 2,
            },
        })
        .collect();
    let title_length = i64::try_from(title.len()).map_err(|_| MojoError::InvalidInput)?;
    render_measured_text(|measuring, output, capacity, written| unsafe {
        prodex_mojo_operational_log_details_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            title.as_ptr() as u64,
            title_length,
            fields.as_ptr() as u64,
            fields.len() as i64,
            measuring,
            output,
            capacity,
            written,
        )
    })
}

#[cfg(test)]
#[path = "details_tests.rs"]
mod tests;
