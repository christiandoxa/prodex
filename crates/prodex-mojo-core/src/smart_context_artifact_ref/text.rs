//! Borrowed artifact text and caller-owned scratch for the compiled text kernel.
use super::ArtifactLineRange;
use crate::MojoError;
use std::borrow::Cow;

#[repr(C)]
#[derive(Default)]
struct Input {
    operation: i64,
    text_address: u64,
    text_length: u64,
    id_address: u64,
    id_length: u64,
    marker_address: u64,
    marker_length: u64,
    replacement_address: u64,
    replacement_length: u64,
    ranges: u64,
    range_count: i64,
    has_line_range: i64,
    scratch: u64,
    scratch_words: i64,
}
const _: () = assert!(std::mem::size_of::<Input>() == 112);

unsafe extern "C" {
    fn prodex_smart_context_artifact_text_v1(
        abi: i64,
        input: u64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
        metadata_count: i64,
    ) -> i64;
}

#[derive(PartialEq)]
struct TextPlan<'a> {
    present: bool,
    bytes: usize,
    stop: bool,
    source: Option<&'a str>,
}

enum Acquisition {
    Length,
    Text,
}

enum Acquired<'a> {
    Length(usize),
    Text(Cow<'a, str>, bool),
}

fn invoke<'a>(
    mut input: Input,
    original: &'a str,
    replacement: &'a str,
    acquisition: Acquisition,
) -> Result<Option<Acquired<'a>>, MojoError> {
    if [
        input.text_length,
        input.id_length,
        input.marker_length,
        input.replacement_length,
    ]
    .into_iter()
    .any(|length| length > 64 * 1024 * 1024)
    {
        return Err(MojoError::InvalidInput);
    }
    let words = usize::try_from(input.scratch_words).map_err(|_| MojoError::Capacity)?;
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(words)
        .map_err(|_| MojoError::Capacity)?;
    scratch.resize(words, 0_i64);
    input.scratch = scratch.as_mut_ptr() as usize as u64;
    let mut metadata = [-1_i64; 4];
    let mut call = |output: Option<&mut [u8]>| -> Result<TextPlan<'_>, MojoError> {
        let (measuring, address, capacity) = match output {
            Some(bytes) => (
                0,
                bytes.as_mut_ptr() as usize as u64,
                i64::try_from(bytes.len()).map_err(|_| MojoError::Capacity)?,
            ),
            None => (1, 0, 0),
        };
        // All borrowed input and range allocations live through both synchronous calls.
        let status = unsafe {
            prodex_smart_context_artifact_text_v1(
                1,
                (&input as *const Input) as usize as u64,
                measuring,
                address,
                capacity,
                metadata.as_mut_ptr() as usize as u64,
                metadata.len() as i64,
            )
        };
        match status {
            0 => {}
            1 => return Err(MojoError::InvalidInput),
            3 => return Err(MojoError::Capacity),
            4 => return Err(MojoError::AbiMismatch),
            _ => return Err(MojoError::InvalidOutput),
        }
        let bytes = usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)?;
        let (present, stop) = match (metadata[0], metadata[2]) {
            (0, 0) if bytes == 0 => (false, false),
            (1, 0) => (true, false),
            (1, 1) if input.operation == 1 => (true, true),
            _ => return Err(MojoError::InvalidOutput),
        };
        let source = match metadata[3] {
            0 => None,
            1 if present && input.operation == 0 && bytes == original.len() => Some(original),
            2 if present && input.operation == 1 && bytes == replacement.len() => Some(replacement),
            _ => return Err(MojoError::InvalidOutput),
        };
        Ok(TextPlan {
            present,
            bytes,
            stop,
            source,
        })
    };
    let plan = call(None)?;
    if !plan.present {
        return Ok(None);
    }
    if matches!(acquisition, Acquisition::Length) {
        return Ok(Some(Acquired::Length(plan.bytes)));
    }
    if let Some(source) = plan.source {
        return Ok(Some(Acquired::Text(Cow::Borrowed(source), plan.stop)));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(plan.bytes)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(plan.bytes, 0);
    if call(Some(&mut output))? != plan {
        return Err(MojoError::InvalidOutput);
    }
    let text = String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)?;
    Ok(Some(Acquired::Text(Cow::Owned(text), plan.stop)))
}

/// Selects one-based inclusive ranges using Rust-compatible line endings.
/// Unavailable ranges return `None`; explicit ranges take precedence over the legacy range.
pub fn rehydrate_artifact_lines<'a>(
    text: &'a str,
    ranges: &[ArtifactLineRange],
    line_range: Option<ArtifactLineRange>,
) -> Result<Option<Cow<'a, str>>, MojoError> {
    artifact_lines(text, ranges, line_range, Acquisition::Text)?
        .map(|result| match result {
            Acquired::Text(text, _) => Ok(text),
            Acquired::Length(_) => Err(MojoError::InvalidOutput),
        })
        .transpose()
}

/// Measures the same compiled line plan without allocating or copying its output.
pub fn artifact_line_byte_len(
    text: &str,
    ranges: &[ArtifactLineRange],
    line_range: Option<ArtifactLineRange>,
) -> Result<Option<usize>, MojoError> {
    artifact_lines(text, ranges, line_range, Acquisition::Length)?
        .map(|result| match result {
            Acquired::Length(length) => Ok(length),
            Acquired::Text(_, _) => Err(MojoError::InvalidOutput),
        })
        .transpose()
}

fn artifact_lines<'a>(
    text: &'a str,
    ranges: &[ArtifactLineRange],
    line_range: Option<ArtifactLineRange>,
    acquisition: Acquisition,
) -> Result<Option<Acquired<'a>>, MojoError> {
    let pairs = line_range
        .into_iter()
        .chain(ranges.iter().copied())
        .flat_map(|range| [range.start as u64, range.end as u64])
        .collect::<Vec<_>>();
    // This byte count sizes host storage; Mojo determines all logical line spans.
    let words = if ranges.is_empty() && line_range.is_none() {
        0
    } else {
        text.bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            .checked_add(1)
            .and_then(|count| count.checked_mul(2))
            .ok_or(MojoError::Capacity)?
    };
    invoke(
        Input {
            operation: 0,
            text_address: text.as_ptr() as usize as u64,
            text_length: text.len() as u64,
            ranges: pairs.as_ptr() as usize as u64,
            range_count: i64::try_from(ranges.len()).map_err(|_| MojoError::InvalidInput)?,
            has_line_range: i64::from(line_range.is_some()),
            scratch_words: i64::try_from(words).map_err(|_| MojoError::Capacity)?,
            ..Input::default()
        },
        text,
        "",
        acquisition,
    )
}

/// Facts for replacing an existing reference in a single decoded text value.
pub struct ArtifactReplacementInput<'a> {
    pub text: &'a str,
    pub id: &'a str,
    pub marker: &'a str,
    pub replacement: &'a str,
    pub has_line_range: bool,
}

/// Whether a whole summary was consumed or later references may still be applied.
#[derive(Debug, PartialEq, Eq)]
pub enum ArtifactReplacement {
    Continue(String),
    Complete(String),
}

/// Applies the compiled summary/marker/legacy precedence and all non-overlapping matches.
pub fn replace_artifact_reference(
    facts: ArtifactReplacementInput<'_>,
) -> Result<Option<ArtifactReplacement>, MojoError> {
    let pattern_bytes = facts
        .id
        .len()
        .checked_mul(2)
        .and_then(|length| length.checked_add(32))
        .ok_or(MojoError::Capacity)?;
    let maximum_pattern = facts
        .marker
        .len()
        .max(facts.id.len().checked_add(16).ok_or(MojoError::Capacity)?);
    let words = pattern_bytes
        .div_ceil(8)
        .checked_add(maximum_pattern)
        .ok_or(MojoError::Capacity)?;
    invoke(
        Input {
            operation: 1,
            text_address: facts.text.as_ptr() as usize as u64,
            text_length: facts.text.len() as u64,
            id_address: facts.id.as_ptr() as usize as u64,
            id_length: facts.id.len() as u64,
            marker_address: facts.marker.as_ptr() as usize as u64,
            marker_length: facts.marker.len() as u64,
            replacement_address: facts.replacement.as_ptr() as usize as u64,
            replacement_length: facts.replacement.len() as u64,
            has_line_range: i64::from(facts.has_line_range),
            scratch_words: i64::try_from(words).map_err(|_| MojoError::Capacity)?,
            ..Input::default()
        },
        facts.text,
        facts.replacement,
        Acquisition::Text,
    )?
    .map(|result| match result {
        Acquired::Text(text, stop) => Ok(if stop {
            ArtifactReplacement::Complete(text.into_owned())
        } else {
            ArtifactReplacement::Continue(text.into_owned())
        }),
        Acquired::Length(_) => Err(MojoError::InvalidOutput),
    })
    .transpose()
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
