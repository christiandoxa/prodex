//! Compiled PID parsing and active-runtime log selection.
//! Rust keeps native paths and applies checked Mojo indices.

use super::structured_log_status;
use crate::{MojoError, json::signed};

unsafe extern "C" {
    fn prodex_runtime_log_pid_v1(
        abi: i64,
        name_address: u64,
        name_length: i64,
        prefix_address: u64,
        prefix_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_active_runtime_logs_v1(
        abi: i64,
        process_address: u64,
        process_count: i64,
        pids_address: u64,
        path_count: i64,
        scratch_address: u64,
        scratch_count: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

/// Host-observed process identity and runtime classification.
#[derive(Debug, Clone, Copy)]
pub struct ActiveRuntimeLogProcess {
    pub pid: u32,
    pub runtime: bool,
}

/// Empty-path queries preserve the caller's lazy iterator boundary.
/// Path indices retain the last input per active PID, ordered by PID.
#[derive(Debug, PartialEq, Eq)]
pub struct ActiveRuntimeLogPlan {
    pub runtime_processes_present: bool,
    pub path_indices: Vec<usize>,
}

/// Parse a native basename using Mojo's exact prefix and u32 rules.
pub fn runtime_log_pid(name: &str, prefix: &str) -> Result<Option<u32>, MojoError> {
    let mut output = [u64::MAX; 2];
    structured_log_status(unsafe {
        prodex_runtime_log_pid_v1(
            1,
            name.as_ptr() as u64,
            signed(name.len())?,
            prefix.as_ptr() as u64,
            signed(prefix.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output {
        [0, 0] => Ok(None),
        [1, pid] => u32::try_from(pid)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Mojo owns membership, last-encountered retention and numeric PID order.
/// Rust encodes optional PID observations and validates the returned indices.
pub fn active_runtime_log_plan(
    processes: &[ActiveRuntimeLogProcess],
    path_pids: &[Option<u32>],
) -> Result<ActiveRuntimeLogPlan, MojoError> {
    let row_words = processes
        .len()
        .checked_mul(2)
        .ok_or(MojoError::InvalidInput)?;
    let scratch_len = processes
        .len()
        .checked_add(path_pids.len())
        .ok_or(MojoError::InvalidInput)?;
    signed(row_words.checked_mul(8).ok_or(MojoError::InvalidInput)?)?;
    signed(scratch_len.checked_mul(8).ok_or(MojoError::InvalidInput)?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_words)
        .map_err(|_| MojoError::Capacity)?;
    for process in processes {
        rows.extend([u64::from(process.pid), u64::from(process.runtime)]);
    }
    // u64::MAX represents absence; every actual PID fits u32.
    let pids = path_pids
        .iter()
        .map(|pid| pid.map_or(u64::MAX, u64::from))
        .collect::<Vec<_>>();
    let mut scratch = vec![-1_i64; scratch_len];
    let mut output = vec![-1_i64; path_pids.len()];
    let mut metadata = [-1_i64; 2];
    structured_log_status(unsafe {
        prodex_active_runtime_logs_v1(
            1,
            rows.as_ptr() as u64,
            signed(processes.len())?,
            pids.as_ptr() as u64,
            signed(pids.len())?,
            scratch.as_mut_ptr() as u64,
            signed(scratch.len())?,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
            metadata.as_mut_ptr() as u64,
        )
    })?;
    let runtime_processes_present = match metadata[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let written = usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() || (!runtime_processes_present && written != 0) {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    let mut seen = vec![false; path_pids.len()];
    let mut path_indices = Vec::with_capacity(written);
    for value in output {
        let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
        let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *used {
            return Err(MojoError::InvalidOutput);
        }
        *used = true;
        path_indices.push(index);
    }
    Ok(ActiveRuntimeLogPlan {
        runtime_processes_present,
        path_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_active_log_plan_preserves_membership_last_input_and_pid_order() {
        let processes = [(200, true), (2, true), (201, false), (200, false)]
            .map(|(pid, runtime)| ActiveRuntimeLogProcess { pid, runtime });
        assert_eq!(
            active_runtime_log_plan(
                &processes,
                &[
                    Some(200),
                    Some(201),
                    Some(2),
                    Some(200),
                    None,
                    Some(999),
                    Some(2)
                ]
            )
            .unwrap(),
            ActiveRuntimeLogPlan {
                runtime_processes_present: true,
                path_indices: vec![6, 3]
            }
        );
        assert_eq!(
            active_runtime_log_plan(&processes, &[]).unwrap(),
            ActiveRuntimeLogPlan {
                runtime_processes_present: true,
                path_indices: vec![]
            }
        );
        assert_eq!(
            active_runtime_log_plan(&[], &[Some(200), None]).unwrap(),
            ActiveRuntimeLogPlan {
                runtime_processes_present: false,
                path_indices: vec![]
            }
        );
    }

    #[test]
    fn compiled_active_log_pid_preserves_optional_plus_and_unsigned_bounds() {
        for (name, expected) in [
            ("logs-+0002-", Some(2)),
            ("logs-0-x", Some(0)),
            ("logs-4294967295-東京", Some(u32::MAX)),
            ("logs-4294967296-x", None),
            ("logs--2-x", None),
            ("logs-2", None),
            ("logs- 2-x", None),
            ("LOGS-2-x", None),
            ("logs-２-x", None),
        ] {
            assert_eq!(runtime_log_pid(name, "logs"), Ok(expected));
        }
    }

    #[test]
    fn compiled_active_log_abi_rejects_late_invalid_pid_without_writes() {
        let rows = [2_u64, 1];
        let pids = [2_u64, u64::from(u32::MAX) + 1];
        let mut scratch = [81_i64; 3];
        let mut output = [83_i64; 2];
        let mut metadata = [77_i64; 2];
        assert_eq!(
            unsafe {
                prodex_active_runtime_logs_v1(
                    1,
                    rows.as_ptr() as u64,
                    1,
                    pids.as_ptr() as u64,
                    2,
                    scratch.as_mut_ptr() as u64,
                    3,
                    output.as_mut_ptr() as u64,
                    2,
                    metadata.as_mut_ptr() as u64,
                )
            },
            1
        );
        assert_eq!(scratch, [81; 3]);
        assert_eq!(output, [83; 2]);
        assert_eq!(metadata, [77; 2]);
        assert_eq!(
            unsafe { prodex_active_runtime_logs_v1(2, 0, -1, 0, -1, 0, 0, 0, 0, 0) },
            4
        );
        let invalid = [0xff_u8];
        let mut result = [79_u64; 2];
        assert_eq!(
            unsafe {
                prodex_runtime_log_pid_v1(
                    1,
                    invalid.as_ptr() as u64,
                    1,
                    b"logs".as_ptr() as u64,
                    4,
                    result.as_mut_ptr() as u64,
                )
            },
            1
        );
        assert_eq!(result, [79; 2]);
    }
}
