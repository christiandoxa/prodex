//! Native observations for the compiled status token-panel presentation.
use super::{StringView, signed, status};
use crate::{MojoError, info_render::TokenUsageCounts, text_output::render_measured_string};

/// Observed token counters and display text; empty timestamps retain their presence.
pub struct StatusTokenPanelInput<'a> {
    pub tokens: TokenUsageCounts,
    pub event_count: usize,
    pub log_count: usize,
    pub profile_count: usize,
    pub runtime_profile: &'a str,
    pub active_profile: &'a str,
    pub first_at: Option<&'a str>,
    pub last_at: Option<&'a str>,
}

#[repr(C)]
struct PanelFfi {
    presence: u64,
    tokens: TokenUsageCounts,
    events: u64,
    logs: u64,
    profiles: u64,
    fields: [StringView; 4],
}
const _: () =
    assert!(std::mem::size_of::<PanelFfi>() == 128 && std::mem::align_of::<PanelFfi>() == 8);

unsafe extern "C" {
    fn prodex_status_token_panel_v1(
        abi: i64,
        input: u64,
        scratch: u64,
        scratch_capacity: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Render styled token rows and their history title without Rust formatting decisions.
/// An absent snapshot uses the compiled loading presentation.
pub fn status_token_panel_json(
    input: Option<StatusTokenPanelInput<'_>>,
) -> Result<String, MojoError> {
    let input = input.map(|input| PanelFfi {
        presence: u64::from(input.first_at.is_some()) | (u64::from(input.last_at.is_some()) << 1),
        tokens: input.tokens,
        events: input.event_count as u64,
        logs: input.log_count as u64,
        profiles: input.profile_count as u64,
        fields: [
            input.runtime_profile.into(),
            input.active_profile.into(),
            input.first_at.unwrap_or_default().into(),
            input.last_at.unwrap_or_default().into(),
        ],
    });
    let mut scratch = [0_u8; 128];
    render_measured_string(|output| {
        let (phase, address, capacity) = match output {
            None => (1, 0, 0),
            Some(output) => (0, output.as_mut_ptr() as u64, signed(output.len())?),
        };
        let mut metadata = [-1_i64; 2];
        // SAFETY: the observed row/strings outlive this synchronous call; numeric
        // scratch, output and metadata are distinct caller-owned allocations.
        status(unsafe {
            prodex_status_token_panel_v1(
                1,
                input
                    .as_ref()
                    .map_or(0, |input| input as *const PanelFfi as u64),
                scratch.as_mut_ptr() as u64,
                128,
                phase,
                address,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        })?;
        if metadata[0] != 1 {
            return Err(MojoError::InvalidOutput);
        }
        usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)
    })
}

#[cfg(test)]
#[path = "status_tokens_tests.rs"]
mod tests;
