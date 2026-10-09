use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_CANONICAL_ID_BYTES: usize = 68;
const MAX_BODY_MARKER_SCAN_BYTES: usize = 4 * 1024 * 1024;

#[repr(i64)]
#[derive(Clone, Copy)]
enum Operation {
    AliasValid = 0,
    AliasDeclaration = 1,
    AliasReference = 2,
    Reference = 3,
    IdValid = 4,
    BodyMarkerPresent = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactLineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactReferencePlan {
    pub id: String,
    pub marker_start: usize,
    pub marker_end: usize,
    pub line_ranges: Vec<ArtifactLineRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactAliasReferencePlan {
    pub alias_start: usize,
    pub alias_end: usize,
    pub marker_start: usize,
    pub marker_end: usize,
    pub line_ranges: Vec<ArtifactLineRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactAliasDeclarationPlan {
    pub alias_start: usize,
    pub alias_end: usize,
    pub id: String,
}

unsafe extern "C" {
    fn prodex_smart_context_artifact_ref_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        ranges_address: u64,
        range_capacity: i64,
        range_count_address: u64,
        meta_address: u64,
    ) -> i64;
}

struct CallResult {
    output: Vec<u8>,
    written: usize,
    ranges: Vec<ArtifactLineRange>,
    meta: [i64; 4],
}

fn parse_ranges(raw: &[i64], count: usize) -> Result<Vec<ArtifactLineRange>, MojoError> {
    raw.as_chunks::<2>()
        .0
        .iter()
        .take(count)
        .map(|pair| {
            let start = usize::try_from(pair[0]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(pair[1]).map_err(|_| MojoError::InvalidOutput)?;
            if start == 0 || end < start {
                return Err(MojoError::InvalidOutput);
            }
            Ok(ArtifactLineRange { start, end })
        })
        .collect()
}

fn call(token: &str, operation: Operation) -> Result<CallResult, MojoError> {
    let range_capacity = token.bytes().filter(|byte| *byte == b',').count() + 1;
    let mut output = vec![0_u8; MAX_CANONICAL_ID_BYTES];
    let mut written = 0_i64;
    let mut raw_ranges = vec![0_i64; range_capacity.saturating_mul(2).max(2)];
    let mut range_count = 0_i64;
    let mut meta = [-1_i64; 4];
    let status = unsafe {
        prodex_smart_context_artifact_ref_v1(
            ABI_VERSION,
            operation as i64,
            token.as_ptr() as usize as u64,
            i64::try_from(token.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            raw_ranges.as_mut_ptr() as usize as u64,
            i64::try_from(range_capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut range_count as *mut i64) as usize as u64,
            meta.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let range_count = usize::try_from(range_count).map_err(|_| MojoError::InvalidOutput)?;
    if range_count > range_capacity {
        return Err(MojoError::InvalidOutput);
    }
    Ok(CallResult {
        output,
        written,
        ranges: parse_ranges(&raw_ranges, range_count)?,
        meta,
    })
}

fn span(
    meta: &[i64; 4],
    start_slot: usize,
    end_slot: usize,
    len: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    if meta[start_slot] < 0 || meta[end_slot] < 0 {
        return Ok(None);
    }
    let start = usize::try_from(meta[start_slot]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(meta[end_slot]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || end > len {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
}

fn output_string(result: &CallResult) -> Result<String, MojoError> {
    String::from_utf8(result.output[..result.written].to_vec())
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn artifact_alias_valid(alias: &str) -> Result<bool, MojoError> {
    let result = call(alias, Operation::AliasValid)?;
    match result.meta[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn artifact_id_valid(id: &str) -> Result<bool, MojoError> {
    let result = call(id, Operation::IdValid)?;
    match result.meta[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Detect known Smart Context artifact markers in a bounded request body.
pub fn body_may_contain_artifact_ref(body: &[u8]) -> Result<bool, MojoError> {
    let body = &body[..body.len().min(MAX_BODY_MARKER_SCAN_BYTES)];
    let mut meta = [-1_i64; 4];
    let mut written = 0_i64;
    let mut range_count = 0_i64;
    let output = [0_u8; 1];
    let mut ranges = [0_i64; 2];
    let status = unsafe {
        prodex_smart_context_artifact_ref_v1(
            ABI_VERSION,
            Operation::BodyMarkerPresent as i64,
            body.as_ptr() as usize as u64,
            i64::try_from(body.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_ptr() as usize as u64,
            0,
            (&mut written as *mut i64) as usize as u64,
            ranges.as_mut_ptr() as usize as u64,
            1,
            (&mut range_count as *mut i64) as usize as u64,
            meta.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => match meta[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn parse_alias_declaration(
    token: &str,
) -> Result<Option<ArtifactAliasDeclarationPlan>, MojoError> {
    let result = call(token, Operation::AliasDeclaration)?;
    let Some((alias_start, alias_end)) = span(&result.meta, 0, 1, token.len())? else {
        return Ok(None);
    };
    Ok(Some(ArtifactAliasDeclarationPlan {
        alias_start,
        alias_end,
        id: output_string(&result)?,
    }))
}

pub fn parse_alias_reference(token: &str) -> Result<Option<ArtifactAliasReferencePlan>, MojoError> {
    let result = call(token, Operation::AliasReference)?;
    let Some((alias_start, alias_end)) = span(&result.meta, 0, 1, token.len())? else {
        return Ok(None);
    };
    let Some((marker_start, marker_end)) = span(&result.meta, 0, 3, token.len())? else {
        return Err(MojoError::InvalidOutput);
    };
    Ok(Some(ArtifactAliasReferencePlan {
        alias_start,
        alias_end,
        marker_start,
        marker_end,
        line_ranges: result.ranges,
    }))
}

pub fn parse_reference(token: &str) -> Result<Option<ArtifactReferencePlan>, MojoError> {
    let result = call(token, Operation::Reference)?;
    let Some((marker_start, marker_end)) = span(&result.meta, 0, 1, token.len())? else {
        return Ok(None);
    };
    Ok(Some(ArtifactReferencePlan {
        id: output_string(&result)?,
        marker_start,
        marker_end,
        line_ranges: result.ranges,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_ref_kernel_smoke() {
        assert!(artifact_alias_valid("@12").unwrap());
        assert!(!artifact_alias_valid("@12.").unwrap());
        assert!(artifact_id_valid("sc:0123456789abcdef").unwrap());
        assert!(!artifact_id_valid("psc:0123456789abcdef").unwrap());
        assert!(body_may_contain_artifact_ref(b"prefix psc:0123456789abcdef suffix").unwrap());
        assert!(!body_may_contain_artifact_ref("Unicode café".as_bytes()).unwrap());

        let short = parse_reference("psc:0123456789abcdef#L2-L4")
            .unwrap()
            .unwrap();
        assert_eq!(short.id, "sc:0123456789abcdef");
        assert_eq!(
            short.line_ranges,
            vec![ArtifactLineRange { start: 2, end: 4 }]
        );

        let long = parse_reference(
            "psc2:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?lines=7,9-11",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            long.id,
            "sc2:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(long.line_ranges.len(), 2);

        let declaration = parse_alias_declaration("@3=psc:0123456789abcdef")
            .unwrap()
            .unwrap();
        assert_eq!(
            &"@3=psc:0123456789abcdef"[declaration.alias_start..declaration.alias_end],
            "@3"
        );
        let quoted_declaration = parse_alias_declaration(r#"@4="psc:0123456789abcdef""#)
            .unwrap()
            .unwrap();
        assert_eq!(quoted_declaration.id, "sc:0123456789abcdef");

        let alias = parse_alias_reference("(@3#L5-L6),").unwrap().unwrap();
        assert_eq!(&"(@3#L5-L6),"[alias.alias_start..alias.alias_end], "@3");
        assert_eq!(
            alias.line_ranges,
            vec![ArtifactLineRange { start: 5, end: 6 }]
        );
        let punctuated = parse_reference("(psc:0123456789abcdef?lines=L2,4-L6),")
            .unwrap()
            .unwrap();
        assert_eq!(punctuated.id, "sc:0123456789abcdef");
        assert_eq!(
            punctuated.line_ranges,
            vec![
                ArtifactLineRange { start: 2, end: 2 },
                ArtifactLineRange { start: 4, end: 6 },
            ]
        );
        assert_eq!(
            &"(psc:0123456789abcdef?lines=L2,4-L6),"
                [punctuated.marker_start..punctuated.marker_end],
            "psc:0123456789abcdef?lines=L2,4-L6"
        );

        assert!(parse_reference("psc:0123456789abcdeg").unwrap().is_none());
        assert!(
            parse_alias_declaration("@x=psc:0123456789abcdef")
                .unwrap()
                .is_none()
        );
    }
}
