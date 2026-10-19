use super::{MojoError, render};

pub(super) const PROVIDER_SUMMARY: i64 = 30;
pub(super) const PROVIDER_CAPABILITIES: i64 = 31;

/// Host enum representation for the four provider routing categories.
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum InfoProviderRoutePolicy {
    NativeCodex = 0,
    ResponsesAdapter = 1,
    ExternalCli = 2,
    Unsupported = 3,
}

/// Non-secret capability facts acquired from the existing provider policy.
#[derive(Debug, Clone, Copy)]
pub struct InfoProviderCapability<'a> {
    pub route_policy: InfoProviderRoutePolicy,
    pub uses_openai_client_format: bool,
    pub quota_shape: &'a str,
}

/// Groups and sorts provider labels through the compiled report owner.
pub fn format_provider_summary(labels: &[&str]) -> Result<String, MojoError> {
    render(PROVIDER_SUMMARY, &[], &[], labels, 0)
}

/// Aggregates routes, client format and sorted quota shapes in Mojo.
pub fn format_provider_capabilities_summary(
    facts: &[InfoProviderCapability<'_>],
) -> Result<String, MojoError> {
    let mut unsigned = Vec::new();
    unsigned
        .try_reserve_exact(facts.len().checked_mul(2).ok_or(MojoError::InvalidInput)?)
        .map_err(|_| MojoError::Capacity)?;
    let labels = facts
        .iter()
        .map(|fact| {
            unsigned.extend([
                fact.route_policy as u64,
                u64::from(fact.uses_openai_client_format),
            ]);
            fact.quota_shape
        })
        .collect::<Vec<_>>();
    render(PROVIDER_CAPABILITIES, &[], &unsigned, &labels, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info_render::{InfoStringView, prodex_terminal_info_render_v1};

    #[test]
    fn compiled_provider_summaries_group_unicode_and_all_capability_categories() {
        assert_eq!(format_provider_summary(&[]).unwrap(), "none");
        assert_eq!(format_provider_capabilities_summary(&[]).unwrap(), "none");
        assert_eq!(
            format_provider_summary(&["z", "é", "a", "z", "\0"]).unwrap(),
            "\0=1, a=1, z=2, é=1"
        );
        let facts = [
            InfoProviderCapability {
                route_policy: InfoProviderRoutePolicy::NativeCodex,
                uses_openai_client_format: true,
                quota_shape: "openai-windows",
            },
            InfoProviderCapability {
                route_policy: InfoProviderRoutePolicy::ResponsesAdapter,
                uses_openai_client_format: true,
                quota_shape: "gemini-buckets",
            },
            InfoProviderCapability {
                route_policy: InfoProviderRoutePolicy::ExternalCli,
                uses_openai_client_format: false,
                quota_shape: "external-status",
            },
            InfoProviderCapability {
                route_policy: InfoProviderRoutePolicy::Unsupported,
                uses_openai_client_format: false,
                quota_shape: "external-status",
            },
            InfoProviderCapability {
                route_policy: InfoProviderRoutePolicy::ResponsesAdapter,
                uses_openai_client_format: true,
                quota_shape: "copilot-monthly",
            },
        ];
        assert_eq!(
            format_provider_capabilities_summary(&facts).unwrap(),
            "routes native=1, adapter=2, external-cli=1, unsupported=1; openai-format=3; quota copilot-monthly=1, external-status=2, gemini-buckets=1, openai-windows=1"
        );
        let labels = (0..2048)
            .map(|index| format!("provider_{index:04}"))
            .collect::<Vec<_>>();
        let references = labels.iter().map(String::as_str).collect::<Vec<_>>();
        let expected = labels
            .iter()
            .map(|label| format!("{label}=1"))
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(format_provider_summary(&references).unwrap(), expected);
    }

    #[test]
    fn compiled_provider_summary_abi_rejects_fact_shapes_before_output() {
        let label = "quota";
        let text = [InfoStringView {
            ptr: label.as_ptr() as u64,
            len: label.len() as u64,
        }];
        let mut output = [83_u64; 32];
        let mut written = 71_i64;
        for facts in [[4_u64, 0], [0, 2]] {
            assert_eq!(
                unsafe {
                    prodex_terminal_info_render_v1(
                        1,
                        PROVIDER_CAPABILITIES,
                        0,
                        0,
                        facts.as_ptr() as u64,
                        2,
                        text.as_ptr() as u64,
                        1,
                        0,
                        output.as_mut_ptr() as u64,
                        256,
                        (&mut written as *mut i64) as u64,
                    )
                },
                1
            );
            assert_eq!(written, 71);
            assert_eq!(output, [83; 32]);
        }
    }
}
