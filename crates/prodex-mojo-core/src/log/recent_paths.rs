//! Compiled ordering of recent session and runtime-info log path candidates.
//! The host supplies filesystem metadata and keeps original PathBufs.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_ROWS: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_recent_log_paths_v1(
        abi: i64,
        mode: i64,
        rows_address: u64,
        count: i64,
        limit: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum RecentLogPathOrderMode {
    /// Original session discovery: millisecond precision, descending
    /// modified time, ascending path, and stable ties.
    SessionMilliseconds = 0,
    /// Original runtime-info path selection: descending SystemTime,
    /// descending path and reverse original order on complete ties.
    RuntimeSystemTime = 1,
}

#[derive(Debug, Clone, Copy)]
pub struct RecentLogPathInput<'a> {
    pub path: &'a [u8],
    pub modified_before_unix_epoch: bool,
    /// For SessionMilliseconds: high 64 bits of u128 epoch millis.
    /// For RuntimeSystemTime: zero.
    pub modified_high: u64,
    /// For SessionMilliseconds: low 64 bits of u128 epoch millis.
    /// For RuntimeSystemTime: absolute seconds from Unix epoch.
    pub modified_low: u64,
    pub modified_nanos: u32,
}

pub fn recent_log_path_order(
    mode: RecentLogPathOrderMode,
    candidates: &[RecentLogPathInput<'_>],
    limit: usize,
) -> Result<Vec<usize>, MojoError> {
    if candidates.len() > MAX_ROWS {
        return Err(MojoError::InvalidInput);
    }
    let limit = limit.min(candidates.len());
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(candidates.len().checked_mul(6).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for candidate in candidates {
        if candidate.modified_nanos >= 1_000_000_000
            || (mode == RecentLogPathOrderMode::SessionMilliseconds
                && (candidate.modified_before_unix_epoch || candidate.modified_nanos != 0))
            || (mode == RecentLogPathOrderMode::RuntimeSystemTime && candidate.modified_high != 0)
        {
            return Err(MojoError::InvalidInput);
        }
        rows.extend([
            candidate.path.as_ptr() as usize as u64,
            u64::try_from(candidate.path.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::from(candidate.modified_before_unix_epoch),
            candidate.modified_high,
            candidate.modified_low,
            u64::from(candidate.modified_nanos),
        ]);
    }
    let mut scratch = vec![-1_i64; candidates.len()];
    let mut output = vec![-1_i64; limit];
    let mut written = -1_i64;
    match unsafe {
        prodex_mojo_recent_log_paths_v1(
            ABI_VERSION,
            mode as i64,
            rows.as_ptr() as usize as u64,
            i64::try_from(candidates.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if usize::try_from(written).map_err(|_| MojoError::InvalidOutput)? != limit {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; candidates.len()];
    let mut indices = Vec::with_capacity(limit);
    for value in output {
        let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
        let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *used {
            return Err(MojoError::InvalidOutput);
        }
        *used = true;
        indices.push(index);
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millis<'a>(path: &'a str, epoch_millis: u128) -> RecentLogPathInput<'a> {
        RecentLogPathInput {
            path: path.as_bytes(),
            modified_before_unix_epoch: false,
            modified_high: (epoch_millis >> 64) as u64,
            modified_low: epoch_millis as u64,
            modified_nanos: 0,
        }
    }
    fn time<'a>(path: &'a str, negative: bool, secs: u64, nanos: u32) -> RecentLogPathInput<'a> {
        RecentLogPathInput {
            path: path.as_bytes(),
            modified_before_unix_epoch: negative,
            modified_high: 0,
            modified_low: secs,
            modified_nanos: nanos,
        }
    }

    #[test]
    fn compiled_recent_log_paths_session_millis_preserves_ascending_path_ties() {
        let cases = [
            millis("z", 2000),
            millis("é", 2001),
            millis("a", 2000),
            millis("雪", 2001),
            millis("a", 2000),
            millis("future", (u64::MAX as u128) + 19),
        ];
        assert_eq!(
            recent_log_path_order(RecentLogPathOrderMode::SessionMilliseconds, &cases, 4),
            Ok(vec![5, 1, 3, 2]),
        );
        assert_eq!(
            recent_log_path_order(RecentLogPathOrderMode::SessionMilliseconds, &cases, 0),
            Ok(vec![]),
        );
        assert_eq!(
            recent_log_path_order(RecentLogPathOrderMode::SessionMilliseconds, &[], 4),
            Ok(vec![]),
        );
        assert_eq!(
            recent_log_path_order(RecentLogPathOrderMode::SessionMilliseconds, &cases, 99),
            Ok(vec![5, 1, 3, 2, 4, 0]),
        );
    }

    #[test]
    fn compiled_recent_log_paths_runtime_reverses_path_ties_and_signed_times() {
        let cases = [
            time("a", false, 2, 1),
            time("z", false, 2, 1),
            time("雪", true, 1, 0),
            time("early", true, 4, 0),
            time("a", false, 2, 1),
            time("young", false, 2, 2),
        ];
        assert_eq!(
            recent_log_path_order(RecentLogPathOrderMode::RuntimeSystemTime, &cases, 6),
            Ok(vec![5, 1, 4, 0, 2, 3]),
        );
        assert_eq!(
            recent_log_path_order(RecentLogPathOrderMode::RuntimeSystemTime, &cases, 3),
            Ok(vec![5, 1, 4]),
        );
    }

    #[test]
    fn compiled_recent_log_paths_abi_rejects_late_malformed_row_without_writes() {
        let mut rows = [0_u64; 12];
        let mut scratch = [81_i64; 2];
        let mut indices = [83_i64; 2];
        let mut written = 97_i64;
        let source = rows.as_ptr() as usize as u64;
        let work = scratch.as_mut_ptr() as usize as u64;
        let output = indices.as_mut_ptr() as usize as u64;
        let count = (&mut written as *mut i64) as usize as u64;
        for (abi, mode, src, length, limit, cap, destination, expected) in [
            (2, 0, source, 2, 2, 2, count, 4),
            (1, 2, source, 2, 2, 2, count, 1),
            (1, 0, 0, 2, 2, 2, count, 1),
            (1, 0, source, -1, 2, 2, count, 1),
            (1, 0, source, 65_537, 2, 2, count, 1),
            (1, 0, source, 2, 3, 2, count, 1),
            (1, 0, source, 2, 2, 1, count, 1),
            (1, 0, source, 2, 2, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_recent_log_paths_v1(
                        abi,
                        mode,
                        src,
                        length,
                        limit,
                        work,
                        2,
                        output,
                        cap,
                        destination,
                    )
                },
                expected,
            );
            assert_eq!(scratch, [81; 2]);
            assert_eq!(indices, [83; 2]);
            assert_eq!(written, 97);
        }
        rows[11] = 1_000_000_000;
        assert_eq!(
            unsafe {
                prodex_mojo_recent_log_paths_v1(
                    1,
                    0,
                    rows.as_ptr() as usize as u64,
                    2,
                    2,
                    work,
                    2,
                    output,
                    2,
                    count,
                )
            },
            1,
        );
        assert_eq!(scratch, [81; 2]);
        assert_eq!(indices, [83; 2]);
        assert_eq!(written, 97);
    }
}
