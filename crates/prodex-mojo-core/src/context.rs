#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ProdexStringView {
    ptr: *const u8,
    len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ProdexBytesView {
    ptr: *const u8,
    len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ContextTextRowsResult {
    abi_version: i64,
    before_line_count: i64,
    after_line_count: i64,
    before_rows_written: i64,
    key_count: i64,
    after_signal_line_count: i64,
    required_before_rows: i64,
    required_key_capacity: i64,
    required_hash_capacity: i64,
}

const _: () = {
    assert!(std::mem::size_of::<ProdexStringView>() == 2 * std::mem::size_of::<usize>());
    assert!(std::mem::align_of::<ProdexStringView>() == std::mem::align_of::<usize>());
    assert!(std::mem::offset_of!(ProdexStringView, ptr) == 0);
    assert!(std::mem::offset_of!(ProdexStringView, len) == std::mem::size_of::<usize>());
    assert!(std::mem::size_of::<ProdexBytesView>() == 2 * std::mem::size_of::<usize>());
    assert!(std::mem::align_of::<ProdexBytesView>() == std::mem::align_of::<usize>());
    assert!(std::mem::offset_of!(ProdexBytesView, ptr) == 0);
    assert!(std::mem::offset_of!(ProdexBytesView, len) == std::mem::size_of::<usize>());
    assert!(std::mem::size_of::<ContextTextRowsResult>() == 9 * std::mem::size_of::<i64>());
    assert!(std::mem::align_of::<ContextTextRowsResult>() == std::mem::align_of::<i64>());
    assert!(std::mem::offset_of!(ContextTextRowsResult, abi_version) == 0);
    assert!(
        std::mem::offset_of!(ContextTextRowsResult, required_hash_capacity)
            == 8 * std::mem::size_of::<i64>()
    );
};

unsafe extern "C" {
    fn prodex_mojo_text_abi_version() -> i64;
    fn prodex_mojo_text_abi_layout(output: *mut u64, output_count: i64) -> i64;
    fn prodex_context_prepare_signal_rows_v1(
        abi_version: i64,
        before_views: *const ProdexStringView,
        before_counts: *const i64,
        before_count: i64,
        after_views: *const ProdexStringView,
        after_counts: *const i64,
        after_count: i64,
        before_rows: *mut i64,
        before_rows_capacity: i64,
        after_available: *mut i64,
        key_capacity: i64,
        hash_slots: *mut i64,
        hash_capacity: i64,
        key_hashes: *mut u64,
        key_sources: *mut i64,
        key_indices: *mut i64,
        result: *mut ContextTextRowsResult,
    ) -> i64;
    fn prodex_context_estimate_tokens(chars: u64, words: u64) -> u64;
    fn prodex_context_signal_diff(
        before: *const i64,
        after: *const i64,
        lost: *mut i64,
        gained: *mut i64,
    ) -> i64;
    fn prodex_context_lost_line_ranges_batch(
        before_rows: *const i64,
        after_available: *mut i64,
        initial_loss: *const i64,
        output_ranges: *mut i64,
        output_count: *mut i64,
        line_count: i64,
        key_count: i64,
        context_lines: i64,
        max_ranges: i64,
        max_range_lines: i64,
    ) -> i64;
}

const CRITICAL_SIGNAL_COUNTER_COUNT: usize = 7;
const CRITICAL_SIGNAL_ROW_WIDTH: usize = 8;
const CRITICAL_SIGNAL_MAX_LINES: usize = 65_536;
const CRITICAL_SIGNAL_MAX_KEYS: usize = 65_536;
const CRITICAL_SIGNAL_MAX_RANGES: usize = 1_024;
pub const CONTEXT_TEXT_ABI_VERSION: i64 = 1;
static CONTEXT_TEXT_ABI_READY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextSignalLine<'a> {
    pub text: &'a str,
    pub counts: [usize; CRITICAL_SIGNAL_COUNTER_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSignalRows {
    pub before_rows: Vec<i64>,
    pub after_available: Vec<i64>,
}

fn counters_to_i64(
    values: &[usize; CRITICAL_SIGNAL_COUNTER_COUNT],
) -> Result<[i64; CRITICAL_SIGNAL_COUNTER_COUNT], crate::MojoError> {
    let mut converted = [0_i64; CRITICAL_SIGNAL_COUNTER_COUNT];
    for (slot, value) in converted.iter_mut().zip(values) {
        *slot = i64::try_from(*value).map_err(|_| crate::MojoError::InvalidInput)?;
    }
    Ok(converted)
}

fn counters_from_i64(
    values: [i64; CRITICAL_SIGNAL_COUNTER_COUNT],
) -> Result<[usize; CRITICAL_SIGNAL_COUNTER_COUNT], crate::MojoError> {
    let mut converted = [0_usize; CRITICAL_SIGNAL_COUNTER_COUNT];
    for (slot, value) in converted.iter_mut().zip(values) {
        *slot = usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)?;
    }
    Ok(converted)
}

pub fn text_abi_version() -> Result<i64, crate::MojoError> {
    let version = unsafe { prodex_mojo_text_abi_version() };
    (version == CONTEXT_TEXT_ABI_VERSION)
        .then_some(version)
        .ok_or(crate::MojoError::AbiMismatch)
}

pub fn text_abi_layout_matches() -> bool {
    let mut mojo = [0_u64; 12];
    let status = unsafe { prodex_mojo_text_abi_layout(mojo.as_mut_ptr(), mojo.len() as i64) };
    let rust = [
        std::mem::size_of::<ProdexStringView>() as u64,
        std::mem::align_of::<ProdexStringView>() as u64,
        std::mem::offset_of!(ProdexStringView, ptr) as u64,
        std::mem::offset_of!(ProdexStringView, len) as u64,
        std::mem::size_of::<ProdexBytesView>() as u64,
        std::mem::align_of::<ProdexBytesView>() as u64,
        std::mem::offset_of!(ProdexBytesView, ptr) as u64,
        std::mem::offset_of!(ProdexBytesView, len) as u64,
        std::mem::size_of::<ContextTextRowsResult>() as u64,
        std::mem::align_of::<ContextTextRowsResult>() as u64,
        std::mem::offset_of!(ContextTextRowsResult, abi_version) as u64,
        std::mem::offset_of!(ContextTextRowsResult, required_hash_capacity) as u64,
    ];
    status == 0 && mojo == rust
}

pub fn prepare_signal_rows(
    before: &[ContextSignalLine<'_>],
    after: &[ContextSignalLine<'_>],
) -> Result<ContextSignalRows, crate::MojoError> {
    if !*CONTEXT_TEXT_ABI_READY
        .get_or_init(|| text_abi_version().is_ok() && text_abi_layout_matches())
    {
        return Err(crate::MojoError::AbiMismatch);
    }
    if before.len() > CRITICAL_SIGNAL_MAX_LINES || after.len() > CRITICAL_SIGNAL_MAX_LINES {
        return Err(crate::MojoError::InvalidInput);
    }
    let before_views = string_views(before);
    let after_views = string_views(after);
    let before_counts = signal_counts(before)?;
    let after_counts = signal_counts(after)?;
    let before_rows_len = before
        .len()
        .checked_mul(CRITICAL_SIGNAL_ROW_WIDTH)
        .ok_or(crate::MojoError::InvalidInput)?;
    let total_lines = before
        .len()
        .checked_add(after.len())
        .ok_or(crate::MojoError::InvalidInput)?;
    let key_capacity = total_lines.min(CRITICAL_SIGNAL_MAX_KEYS);
    let hash_capacity = key_capacity.saturating_mul(2).max(1).next_power_of_two();
    let mut before_rows = vec![0_i64; before_rows_len];
    let mut after_available = vec![0_i64; key_capacity];
    let mut hash_slots = vec![-1_i64; hash_capacity];
    let mut key_hashes = vec![0_u64; key_capacity];
    let mut key_sources = vec![0_i64; key_capacity];
    let mut key_indices = vec![0_i64; key_capacity];
    let mut result = ContextTextRowsResult::default();
    let status = unsafe {
        prodex_context_prepare_signal_rows_v1(
            CONTEXT_TEXT_ABI_VERSION,
            before_views.as_ptr(),
            before_counts.as_ptr(),
            i64::try_from(before.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            after_views.as_ptr(),
            after_counts.as_ptr(),
            i64::try_from(after.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            before_rows.as_mut_ptr(),
            i64::try_from(before_rows.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            after_available.as_mut_ptr(),
            i64::try_from(key_capacity).map_err(|_| crate::MojoError::InvalidInput)?,
            hash_slots.as_mut_ptr(),
            i64::try_from(hash_capacity).map_err(|_| crate::MojoError::InvalidInput)?,
            key_hashes.as_mut_ptr(),
            key_sources.as_mut_ptr(),
            key_indices.as_mut_ptr(),
            &mut result,
        )
    };
    if status == 4 {
        return Err(crate::MojoError::AbiMismatch);
    }
    if status != 0 {
        return Err(crate::MojoError::InvalidInput);
    }
    let key_count =
        usize::try_from(result.key_count).map_err(|_| crate::MojoError::InvalidOutput)?;
    let expected_after_signals = after
        .iter()
        .filter(|line| line.counts.iter().any(|count| *count > 0))
        .count();
    if result.abi_version != CONTEXT_TEXT_ABI_VERSION
        || result.before_line_count != before.len() as i64
        || result.after_line_count != after.len() as i64
        || result.before_rows_written != before_rows_len as i64
        || result.required_before_rows != before_rows_len as i64
        || result.required_key_capacity != key_capacity as i64
        || result.required_hash_capacity != hash_capacity as i64
        || result.after_signal_line_count != expected_after_signals as i64
        || key_count > key_capacity
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    validate_signal_rows(before, &before_rows, key_count)?;
    after_available.truncate(key_count);
    if after_available.iter().any(|count| *count < 0)
        || after_available.iter().sum::<i64>() != expected_after_signals as i64
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(ContextSignalRows {
        before_rows,
        after_available,
    })
}

fn string_views(lines: &[ContextSignalLine<'_>]) -> Vec<ProdexStringView> {
    lines
        .iter()
        .map(|line| ProdexStringView {
            ptr: line.text.as_ptr(),
            len: line.text.len(),
        })
        .collect()
}

fn signal_counts(lines: &[ContextSignalLine<'_>]) -> Result<Vec<i64>, crate::MojoError> {
    let mut values = Vec::with_capacity(lines.len() * CRITICAL_SIGNAL_COUNTER_COUNT);
    for line in lines {
        values.extend(counters_to_i64(&line.counts)?);
    }
    Ok(values)
}

fn validate_signal_rows(
    before: &[ContextSignalLine<'_>],
    rows: &[i64],
    key_count: usize,
) -> Result<(), crate::MojoError> {
    for (line, row) in before
        .iter()
        .zip(rows.as_chunks::<CRITICAL_SIGNAL_ROW_WIDTH>().0)
    {
        let has_signal = line.counts.iter().any(|count| *count > 0);
        if (has_signal && !(0..key_count as i64).contains(&row[0]))
            || (!has_signal && row[0] != -1)
            || row[1..]
                != counters_to_i64(&line.counts).map_err(|_| crate::MojoError::InvalidOutput)?
        {
            return Err(crate::MojoError::InvalidOutput);
        }
    }
    Ok(())
}

pub fn estimate_tokens(chars: usize, words: usize) -> Result<usize, crate::MojoError> {
    let chars = u64::try_from(chars).map_err(|_| crate::MojoError::InvalidInput)?;
    let words = u64::try_from(words).map_err(|_| crate::MojoError::InvalidInput)?;
    let tokens = unsafe { prodex_context_estimate_tokens(chars, words) };
    usize::try_from(tokens).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn signal_diff(
    before: &[usize; 7],
    after: &[usize; 7],
) -> Result<([usize; 7], [usize; 7]), crate::MojoError> {
    let before = counters_to_i64(before)?;
    let after = counters_to_i64(after)?;
    let mut lost = [0_i64; 7];
    let mut gained = [0_i64; 7];
    let status = unsafe {
        prodex_context_signal_diff(
            before.as_ptr(),
            after.as_ptr(),
            lost.as_mut_ptr(),
            gained.as_mut_ptr(),
        )
    };
    if status != 0 || lost.iter().any(|value| *value < 0) || gained.iter().any(|value| *value < 0) {
        return Err(crate::MojoError::InvalidOutput);
    }
    let lost = counters_from_i64(lost)?;
    let gained = counters_from_i64(gained)?;
    Ok((lost, gained))
}

pub fn lost_line_ranges_batch(
    before_rows: &[i64],
    after_available: &mut [i64],
    initial_loss: &[usize; CRITICAL_SIGNAL_COUNTER_COUNT],
    line_count: usize,
    context_lines: usize,
    max_ranges: usize,
    max_range_lines: usize,
) -> Result<Vec<(usize, usize)>, crate::MojoError> {
    if line_count > CRITICAL_SIGNAL_MAX_LINES
        || after_available.len() > CRITICAL_SIGNAL_MAX_KEYS
        || max_ranges > CRITICAL_SIGNAL_MAX_RANGES
        || context_lines > CRITICAL_SIGNAL_MAX_LINES
        || max_range_lines > CRITICAL_SIGNAL_MAX_LINES
        || before_rows.len()
            != line_count
                .checked_mul(CRITICAL_SIGNAL_ROW_WIDTH)
                .ok_or(crate::MojoError::InvalidInput)?
        || before_rows
            .as_chunks::<CRITICAL_SIGNAL_ROW_WIDTH>()
            .0
            .iter()
            .any(|row| row[0] < -1 || row[1..].iter().any(|value| *value < 0))
        || after_available.iter().any(|value| *value < 0)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    if max_ranges == 0 {
        return Ok(Vec::new());
    }

    let initial_loss = counters_to_i64(initial_loss)?;
    let mut output_ranges = vec![
        0_i64;
        max_ranges
            .checked_mul(2)
            .ok_or(crate::MojoError::InvalidInput)?
    ];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_context_lost_line_ranges_batch(
            before_rows.as_ptr(),
            after_available.as_mut_ptr(),
            initial_loss.as_ptr(),
            output_ranges.as_mut_ptr(),
            &mut output_count,
            i64::try_from(line_count).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(after_available.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(context_lines).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(max_ranges).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(max_range_lines).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || output_count < 0 || output_count as usize > max_ranges {
        return Err(crate::MojoError::InvalidOutput);
    }
    let output_len = usize::try_from(output_count)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .ok_or(crate::MojoError::InvalidOutput)?;
    let mut ranges = Vec::with_capacity(output_len / 2);
    for pair in output_ranges[..output_len].as_chunks::<2>().0 {
        let start = usize::try_from(pair[0]).map_err(|_| crate::MojoError::InvalidOutput)?;
        let end = usize::try_from(pair[1]).map_err(|_| crate::MojoError::InvalidOutput)?;
        if start == 0 || start > line_count || end < start || end > line_count {
            return Err(crate::MojoError::InvalidOutput);
        }
        ranges.push((start, end));
    }
    Ok(ranges)
}

pub fn self_test() -> bool {
    let mut after_available = [0_i64];
    let signal = ContextSignalLine {
        text: "error: 火\0🔥",
        counts: [1, 0, 0, 0, 0, 0, 0],
    };
    let noise = ContextSignalLine {
        text: "noise🙂",
        counts: [0; 7],
    };
    let text_ok = text_abi_version() == Ok(CONTEXT_TEXT_ABI_VERSION)
        && text_abi_layout_matches()
        && prepare_signal_rows(&[signal, signal, noise], &[signal]).is_ok_and(|rows| {
            rows.after_available == [1]
                && rows
                    .before_rows
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|row| row[0])
                    .eq([0, 0, -1])
        });
    text_ok
        && signal_diff(&[3, 0, 4, 1, 0, 2, 8], &[1, 2, 4, 0, 3, 0, 9]).is_ok_and(
            |(lost, gained)| {
                lost == [2, 0, 0, 1, 0, 2, 0]
                    && gained == [0, 2, 0, 0, 3, 0, 1]
                    && estimate_tokens(5, 2) == Ok(3)
                    && lost_line_ranges_batch(
                        &[0, 1, 0, 0, 0, 0, 0, 0],
                        &mut after_available,
                        &[1, 0, 0, 0, 0, 0, 0],
                        1,
                        0,
                        1,
                        1,
                    ) == Ok(vec![(1, 1)])
            },
        )
}

#[cfg(all(test, feature = "mojo-runtime"))]
#[test]
fn signal_diff_self_test_passes() {
    assert!(self_test());
}

#[cfg(all(test, feature = "mojo-runtime"))]
#[test]
fn signal_diff_matches_rust_oracle_for_generated_counters() {
    let mut state = 0x637269746963616c_u64;
    for case in 0..2_000 {
        let before = std::array::from_fn(|_| (next_random(&mut state) % 10_000) as usize);
        let after = std::array::from_fn(|_| (next_random(&mut state) % 10_000) as usize);
        let expected_lost = std::array::from_fn(|index| before[index].saturating_sub(after[index]));
        let expected_gained =
            std::array::from_fn(|index| after[index].saturating_sub(before[index]));
        let actual = signal_diff(&before, &after).expect("generated signal counters are valid");
        assert_eq!(
            actual,
            (expected_lost, expected_gained),
            "signal case {case}"
        );
    }
}

#[cfg(all(test, feature = "mojo-runtime"))]
fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

#[cfg(all(test, feature = "mojo-runtime"))]
mod text_abi_tests {
    use super::{
        CONTEXT_TEXT_ABI_VERSION, ContextSignalLine, ContextTextRowsResult, ProdexStringView,
        prepare_signal_rows, prodex_context_prepare_signal_rows_v1, text_abi_layout_matches,
        text_abi_version,
    };

    #[test]
    fn text_abi_accepts_utf8_embedded_nul_empty_and_unsentinelled_views() {
        assert_eq!(text_abi_version(), Ok(CONTEXT_TEXT_ABI_VERSION));
        assert!(text_abi_layout_matches());

        let unicode = "账户🙂e\u{301}\0東京";
        assert_eq!(raw_status(unicode.as_ptr(), unicode.len(), 8).0, 0);
        assert_eq!(raw_status(std::ptr::null(), 0, 8).0, 0);

        let no_sentinel = [b'e', b'r', b'r', b'o', 0xff];
        assert_eq!(raw_status(no_sentinel.as_ptr(), 4, 8).0, 0);
    }

    #[test]
    fn text_abi_rejects_malformed_utf8_null_nonempty_and_short_output() {
        for malformed in [
            &[0x80][..],
            &[0xc0, 0xaf],
            &[0xe0, 0x80, 0x80],
            &[0xed, 0xa0, 0x80],
            &[0xf0, 0x90, 0x80],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xff],
        ] {
            assert_eq!(raw_status(malformed.as_ptr(), malformed.len(), 8).0, 2);
        }
        assert_eq!(raw_status("🔥".as_ptr(), 3, 8).0, 2);
        assert_eq!(raw_status(std::ptr::null(), 1, 8).0, 2);
        assert_eq!(raw_status_version(0, b"error".as_ptr(), 5, 8).0, 4);

        let (status, result) = raw_status(b"error".as_ptr(), 5, 7);
        assert_eq!(status, 1);
        assert_eq!(result.required_before_rows, 8);
        assert_eq!(result.required_key_capacity, 1);
        assert_eq!(result.required_hash_capacity, 2);
    }

    #[test]
    fn text_pipeline_is_reentrant_across_concurrent_calls() {
        let threads = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    let signal = ContextSignalLine {
                        text: "error: 并行🙂\0",
                        counts: [1, 0, 0, 0, 0, 0, 0],
                    };
                    for _ in 0..100 {
                        let rows = prepare_signal_rows(&[signal, signal], &[signal]).unwrap();
                        assert_eq!(rows.after_available, [1]);
                        assert_eq!(
                            rows.before_rows
                                .as_chunks::<8>()
                                .0
                                .iter()
                                .map(|row| row[0])
                                .collect::<Vec<_>>(),
                            [0, 0]
                        );
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
    }

    fn raw_status(
        ptr: *const u8,
        len: usize,
        before_rows_capacity: usize,
    ) -> (i64, ContextTextRowsResult) {
        raw_status_version(CONTEXT_TEXT_ABI_VERSION, ptr, len, before_rows_capacity)
    }

    fn raw_status_version(
        abi_version: i64,
        ptr: *const u8,
        len: usize,
        before_rows_capacity: usize,
    ) -> (i64, ContextTextRowsResult) {
        let before_views = [ProdexStringView { ptr, len }];
        let before_counts = [1_i64, 0, 0, 0, 0, 0, 0];
        let after_views: [ProdexStringView; 0] = [];
        let after_counts: [i64; 0] = [];
        let mut before_rows = vec![0_i64; before_rows_capacity.max(1)];
        let mut after_available = [0_i64; 1];
        let mut hash_slots = [-1_i64; 2];
        let mut key_hashes = [0_u64; 1];
        let mut key_sources = [0_i64; 1];
        let mut key_indices = [0_i64; 1];
        let mut result = ContextTextRowsResult::default();
        let status = unsafe {
            prodex_context_prepare_signal_rows_v1(
                abi_version,
                before_views.as_ptr(),
                before_counts.as_ptr(),
                1,
                after_views.as_ptr(),
                after_counts.as_ptr(),
                0,
                before_rows.as_mut_ptr(),
                before_rows_capacity as i64,
                after_available.as_mut_ptr(),
                1,
                hash_slots.as_mut_ptr(),
                2,
                key_hashes.as_mut_ptr(),
                key_sources.as_mut_ptr(),
                key_indices.as_mut_ptr(),
                &mut result,
            )
        };
        (status, result)
    }
}
