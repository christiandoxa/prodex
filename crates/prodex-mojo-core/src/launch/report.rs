use crate::{
    MojoError,
    json::{StringView, render_native_text, signed},
};

/// Address text and port observed from the same host SocketAddr.
#[derive(Clone, Copy)]
pub struct RuntimeLaunchReportProxy<'a> {
    pub address: &'a str,
    pub mount: &'a str,
    pub port: u16,
}

/// Observed diagnostics after the host's existing OS display and redaction.
pub struct RuntimeLaunchReportInput<'a> {
    pub flow: &'a str,
    pub binary: &'a str,
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub proxy: Option<RuntimeLaunchReportProxy<'a>>,
    pub args: &'a [String],
    pub env: &'a [(String, String)],
    pub removed_env: &'a [String],
}

/// Observed application fields appended after the shared launch report.
pub struct RuntimeLaunchReportCompletion<'a> {
    pub report: &'a str,
    pub configured_profile: bool,
    pub extra_report: Option<&'a str>,
    pub presidio_redaction_enabled: bool,
}

#[repr(i64)]
enum ReportOperation {
    Report = 0,
    Completion = 1,
}

#[derive(Default)]
struct ReportShape {
    flags: u64,
    args_count: i64,
    env_count: i64,
    removed_count: i64,
    proxy_port: i64,
}

unsafe extern "C" {
    fn prodex_runtime_launch_report_v1(
        abi: i64,
        operation: i64,
        texts: u64,
        text_count: i64,
        flags: u64,
        args_count: i64,
        env_count: i64,
        removed_count: i64,
        proxy_port: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Render the complete plain report; Mojo owns defaults, sections and spacing.
pub fn render_runtime_launch_report(
    input: RuntimeLaunchReportInput<'_>,
) -> Result<String, MojoError> {
    let mut texts = [
        input.flow,
        input.binary,
        input.provider.unwrap_or_default(),
        input.model.unwrap_or_default(),
        input.proxy.map_or("", |proxy| proxy.address),
        input.proxy.map_or("", |proxy| proxy.mount),
    ]
    .map(StringView::from)
    .to_vec();
    texts.extend(
        input
            .args
            .iter()
            .map(|text| StringView::from(text.as_str())),
    );
    for (key, value) in input.env {
        texts.extend([
            StringView::from(key.as_str()),
            StringView::from(value.as_str()),
        ]);
    }
    texts.extend(
        input
            .removed_env
            .iter()
            .map(|text| StringView::from(text.as_str())),
    );
    render_report(
        ReportOperation::Report,
        &texts,
        ReportShape {
            flags: u64::from(input.provider.is_some())
                | (u64::from(input.model.is_some()) << 1)
                | (u64::from(input.proxy.is_some()) << 2),
            args_count: signed(input.args.len())?,
            env_count: signed(input.env.len())?,
            removed_count: signed(input.removed_env.len())?,
            proxy_port: input.proxy.map_or(0, |proxy| i64::from(proxy.port)),
        },
    )
}

/// Append profile, optional extra text and Presidio rows without trimming.
pub fn complete_runtime_launch_report(
    input: RuntimeLaunchReportCompletion<'_>,
) -> Result<String, MojoError> {
    let mut texts = vec![StringView::from(input.report)];
    if let Some(extra) = input.extra_report {
        texts.push(extra.into());
    }
    render_report(
        ReportOperation::Completion,
        &texts,
        ReportShape {
            flags: u64::from(input.configured_profile)
                | (u64::from(input.presidio_redaction_enabled) << 1)
                | (u64::from(input.extra_report.is_some()) << 2),
            ..ReportShape::default()
        },
    )
}

fn render_report(
    operation: ReportOperation,
    texts: &[StringView],
    shape: ReportShape,
) -> Result<String, MojoError> {
    let operation = operation as i64;
    let count = signed(texts.len())?;
    render_native_text(|phase, output, capacity, metadata| {
        // SAFETY: all views and borrowed strings outlive both synchronous calls;
        // shared native allocation owns distinct output and metadata storage.
        unsafe {
            prodex_runtime_launch_report_v1(
                1,
                operation,
                texts.as_ptr() as u64,
                count,
                shape.flags,
                shape.args_count,
                shape.env_count,
                shape.removed_count,
                shape.proxy_port,
                phase,
                output,
                capacity,
                metadata,
            )
        }
    })
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
