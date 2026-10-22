//! Native observed fields for the compiled session TUI presentation.
use super::{StringView, signed, status};
use crate::{MojoError, text_output::render_measured_string};

/// Existing inline and scrollable session-list presentations.
#[derive(Clone, Copy)]
#[repr(i64)]
pub enum SessionTuiMode {
    Inline = 0,
    Scroll = 1,
}

/// Borrowed observed display fields; empty values retain their presence.
pub struct SessionTuiRow<'a> {
    pub id: &'a str,
    pub title: Option<&'a str>,
    pub updated_at: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub profile: Option<&'a str>,
    pub provider: Option<&'a str>,
}

#[repr(C)]
struct RowFfi {
    presence: u64,
    id: StringView,
    fields: [StringView; 5],
}

const _: () = assert!(std::mem::size_of::<RowFfi>() == 104 && std::mem::align_of::<RowFfi>() == 8);

unsafe extern "C" {
    fn prodex_session_tui_rows_v1(
        abi: i64,
        mode: i64,
        rows: u64,
        count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Render ordered reports/lines/spans as nested arrays of [tone, text] tuples.
/// Rust terminal adapters decode the typed tones and apply their existing styles.
pub fn session_tui_rows_json(
    rows: &[SessionTuiRow<'_>],
    mode: SessionTuiMode,
) -> Result<String, MojoError> {
    let count = signed(rows.len())?;
    let mut input = Vec::new();
    input
        .try_reserve_exact(rows.len())
        .map_err(|_| MojoError::Capacity)?;
    for row in rows {
        let fields = [
            row.title,
            row.updated_at,
            row.cwd,
            row.profile,
            row.provider,
        ];
        let presence = fields
            .iter()
            .enumerate()
            .fold(0_u64, |mask, (index, field)| {
                mask | (u64::from(field.is_some()) << index)
            });
        input.push(RowFfi {
            presence,
            id: row.id.into(),
            fields: fields.map(|field| field.unwrap_or_default().into()),
        });
    }
    // Existing parsed-tree and measured-text runners impose a new 64 MiB limit here.
    render_measured_string(|output| {
        let (phase, address, capacity) = match output {
            None => (1, 0, 0),
            Some(output) => (0, output.as_mut_ptr() as u64, signed(output.len())?),
        };
        let mut metadata = [-1_i64; 2];
        // SAFETY: borrowed strings and rows outlive this synchronous call; output
        // and metadata are distinct host allocations. Mojo validates all fields.
        status(unsafe {
            prodex_session_tui_rows_v1(
                1,
                mode as i64,
                input.as_ptr() as u64,
                count,
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
#[path = "session_tui_tests.rs"]
mod tests;
