//! Borrowed provider observations for complete quota panel schemas.
use super::{StringView, render_native_text, signed};
use crate::MojoError;

#[derive(Clone, Copy)]
#[repr(i64)]
pub enum QuotaPanelKind {
    OpenAi,
    Copilot,
    Gemini,
    External,
}

/// Additional OpenAI names in provider precedence order; External uses the first
/// name as its verbatim label, and Gemini supplies only the formatted bucket value.
#[derive(Default)]
pub struct QuotaPanelExtra<'a> {
    pub names: [Option<&'a str>; 3],
    pub window_label: &'a str,
    pub value: &'a str,
}

/// Existing provider formatters supply values; Mojo owns defaults and row choices.
#[derive(Default)]
pub struct QuotaPanelInput<'a> {
    pub profile: &'a str,
    pub provider: Option<&'a str>,
    pub account: Option<&'a str>,
    pub plan: Option<&'a str>,
    pub access: Option<&'a str>,
    pub project: Option<&'a str>,
    pub status: &'a str,
    pub main: &'a str,
    pub reset: Option<&'a str>,
    pub code_review: Option<&'a str>,
    pub reset_credits: Option<&'a str>,
    pub extras: &'a [QuotaPanelExtra<'a>],
}

#[repr(C)]
struct ExtraRow {
    presence: u64,
    fields: [StringView; 5],
}
const _: () =
    assert!(std::mem::size_of::<ExtraRow>() == 88 && std::mem::align_of::<ExtraRow>() == 8);

unsafe extern "C" {
    fn prodex_quota_profile_panel_v1(
        abi: i64,
        kind: i64,
        detail: i64,
        fields: u64,
        presence: u64,
        extras: u64,
        extra_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Return the panel title and ordered label/value rows as measured JSON.
pub fn quota_profile_panel_json(
    kind: QuotaPanelKind,
    input: QuotaPanelInput<'_>,
    detail: bool,
) -> Result<String, MojoError> {
    let values = [
        Some(input.profile),
        input.provider,
        input.account,
        input.plan,
        input.access,
        input.project,
        Some(input.status),
        Some(input.main),
        input.reset,
        input.code_review,
        input.reset_credits,
    ];
    let presence = values
        .iter()
        .enumerate()
        .fold(0_u64, |mask, (index, value)| {
            mask | (u64::from(value.is_some()) << index)
        });
    let fields = values.map(|value| StringView::from(value.unwrap_or_default()));
    let extras = input
        .extras
        .iter()
        .map(|extra| ExtraRow {
            presence: extra
                .names
                .iter()
                .enumerate()
                .fold(0, |mask, (index, value)| {
                    mask | (u64::from(value.is_some()) << index)
                }),
            fields: [
                extra.names[0].unwrap_or_default().into(),
                extra.names[1].unwrap_or_default().into(),
                extra.names[2].unwrap_or_default().into(),
                extra.window_label.into(),
                extra.value.into(),
            ],
        })
        .collect::<Vec<_>>();
    let extra_count = signed(extras.len())?;
    render_native_text(|phase, output, capacity, metadata| unsafe {
        // All borrowed views outlive both calls; output and metadata are distinct.
        prodex_quota_profile_panel_v1(
            1,
            kind as i64,
            i64::from(detail),
            fields.as_ptr() as u64,
            presence,
            extras.as_ptr() as u64,
            extra_count,
            phase,
            output,
            capacity,
            metadata,
        )
    })
}

#[cfg(test)]
#[path = "quota_profile_panel_tests.rs"]
mod tests;
