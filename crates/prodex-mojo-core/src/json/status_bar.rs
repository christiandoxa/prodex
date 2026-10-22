use super::{MojoError, StringView, render_native_text, signed};

/// Snapshot text observed by the status header; empty text remains present.
pub struct StatusHeaderInput<'a> {
    pub profile: &'a str,
    pub updated_at: &'a str,
}

#[repr(i64)]
enum StatusBar {
    Header = 0,
    Footer = 1,
}

unsafe extern "C" {
    fn prodex_status_bar_v1(
        abi: i64,
        operation: i64,
        texts: u64,
        text_count: i64,
        presence: u64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Render the header's styled spans; Mojo chooses loading/live/refreshing text.
pub fn status_header_json(
    snapshot: Option<StatusHeaderInput<'_>>,
    refreshing: bool,
) -> Result<String, MojoError> {
    let presence = u64::from(snapshot.is_some()) | (u64::from(refreshing) << 1);
    let texts = snapshot.map(|snapshot| [snapshot.profile, snapshot.updated_at]);
    render_status_bar(
        StatusBar::Header,
        texts.as_ref().map_or(&[], |texts| texts.as_slice()),
        presence,
    )
}

/// Render footer hints or an observed error, preserving empty error presence.
pub fn status_footer_json(error: Option<&str>) -> Result<String, MojoError> {
    let texts = error.map(|error| [error]);
    render_status_bar(
        StatusBar::Footer,
        texts.as_ref().map_or(&[], |texts| texts.as_slice()),
        u64::from(error.is_some()),
    )
}

fn render_status_bar(kind: StatusBar, texts: &[&str], presence: u64) -> Result<String, MojoError> {
    let operation = kind as i64;
    let count = signed(texts.len())?;
    let texts = texts
        .iter()
        .map(|text| StringView::from(*text))
        .collect::<Vec<_>>();
    render_native_text(|phase, output, capacity, metadata| {
        // SAFETY: views and borrowed text outlive the synchronous call, and the
        // shared native renderer owns distinct output and metadata allocations.
        unsafe {
            prodex_status_bar_v1(
                1,
                operation,
                texts.as_ptr() as u64,
                count,
                presence,
                phase,
                output,
                capacity,
                metadata,
            )
        }
    })
}

#[cfg(test)]
#[path = "status_bar_tests.rs"]
mod tests;
