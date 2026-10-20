//! Production Copilot custom-instruction discovery decisions in compiled
//! launch Mojo. Host owns directory metadata, symlink checks and file bytes.
//! Output selection and UTF-8 byte clipping are compiled policies.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_PATHS: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_launch_instruction_path_order_v1(
        version: i64,
        views_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_launch_instruction_text_cut_v1(
        version: i64,
        text_address: u64,
        text_length: i64,
        max_bytes: i64,
        end_address: u64,
    ) -> i64;
}

fn check_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Sort original directory path byte views, returning indices in exact
/// lexicographic byte order, with stable input order for equal values.
pub fn launch_instruction_path_order(paths: &[&[u8]]) -> Result<Vec<usize>, MojoError> {
    if paths.len() > MAX_PATHS {
        return Err(MojoError::InvalidInput);
    }
    let mut words = Vec::<u64>::new();
    words
        .try_reserve_exact(paths.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for path in paths {
        words.extend([
            path.as_ptr() as usize as u64,
            u64::try_from(path.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut indices = vec![-1_i64; paths.len()];
    check_status(unsafe {
        prodex_launch_instruction_path_order_v1(
            ABI_VERSION,
            words.as_ptr() as usize as u64,
            i64::try_from(paths.len()).map_err(|_| MojoError::InvalidInput)?,
            indices.as_mut_ptr() as usize as u64,
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    })?;
    let mut seen = vec![false; paths.len()];
    indices
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            let prior = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *prior {
                return Err(MojoError::InvalidOutput);
            }
            *prior = true;
            Ok(index)
        })
        .collect()
}

/// Cut a borrowed post-lossy-UTF8 String on an exact scalar boundary
/// without reproducing the boundary search in the Rust host.
pub fn launch_instruction_text_cut(text: &str, max_bytes: usize) -> Result<usize, MojoError> {
    let mut end = -1_i64;
    check_status(unsafe {
        prodex_launch_instruction_text_cut_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            (&mut end as *mut i64) as usize as u64,
        )
    })?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if end > text.len() || end > max_bytes || !text.is_char_boundary(end) {
        return Err(MojoError::InvalidOutput);
    }
    if text.len() <= max_bytes && end != text.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_launch_instruction_path_order_preserves_unicode_and_equal_ties() {
        let paths = [
            "雪.instructions.md".as_bytes(),
            "z.instructions.md".as_bytes(),
            "a.instructions.md".as_bytes(),
            "é.instructions.md".as_bytes(),
            "a.instructions.md".as_bytes(),
        ];
        assert_eq!(
            launch_instruction_path_order(&paths),
            Ok(vec![2, 4, 1, 3, 0])
        );
        assert_eq!(launch_instruction_path_order(&[]), Ok(vec![]));
        let big = (0..1025)
            .rev()
            .map(|index| format!("nested/{index:04}.instructions.md"))
            .collect::<Vec<_>>();
        let paths = big.iter().map(String::as_bytes).collect::<Vec<_>>();
        let selected = launch_instruction_path_order(&paths).unwrap();
        assert_eq!(selected.len(), 1025);
        assert_eq!(selected[0], 1024);
        assert_eq!(selected[1024], 0);
        assert!(
            selected
                .windows(2)
                .all(|pair| paths[pair[0]] <= paths[pair[1]])
        );
    }

    #[test]
    fn compiled_launch_instruction_text_cut_preserves_late_utf8_boundaries() {
        let sample = "abé🙂";
        for (max, expected) in [
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 2),
            (4, 4),
            (5, 4),
            (6, 4),
            (7, 4),
            (8, 8),
            (9, 8),
        ] {
            assert_eq!(
                launch_instruction_text_cut(sample, max),
                Ok(expected),
                "{max}"
            );
        }
        assert_eq!(launch_instruction_text_cut("", 0), Ok(0));
        assert_eq!(launch_instruction_text_cut("雪雪", 4), Ok(3));
        assert_eq!(launch_instruction_text_cut("🦀", 1), Ok(0));
    }

    #[test]
    fn compiled_launch_instruction_abis_reject_malformed_late_fields_without_writes() {
        let mut views = [0_u64; 4];
        let mut output = [71_i64; 2];
        let source = views.as_ptr() as usize as u64;
        let target = output.as_mut_ptr() as usize as u64;
        for (version, source, count, capacity, expected) in [
            (2, source, 2, 2, 4),
            (1, 0, 2, 2, 1),
            (1, source, -1, 2, 1),
            (1, source, 1_048_577, 2, 1),
            (1, source, 2, 1, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_launch_instruction_path_order_v1(
                        version, source, count, target, capacity,
                    )
                },
                expected,
            );
            assert_eq!(output, [71; 2]);
        }
        views[3] = u64::MAX;
        assert_eq!(
            unsafe {
                prodex_launch_instruction_path_order_v1(
                    1,
                    views.as_ptr() as usize as u64,
                    2,
                    target,
                    2,
                )
            },
            1,
        );
        assert_eq!(output, [71; 2]);
        let text = b"abc";
        let mut end = 83_i64;
        let end_ptr = (&mut end as *mut i64) as usize as u64;
        for (version, text_address, len, limit, out, expected) in [
            (2, text.as_ptr() as usize as u64, 3, 1, end_ptr, 4),
            (1, 0, 3, 1, end_ptr, 1),
            (1, text.as_ptr() as usize as u64, -1, 1, end_ptr, 1),
            (1, text.as_ptr() as usize as u64, 3, -1, end_ptr, 1),
            (1, text.as_ptr() as usize as u64, 3, 1, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_launch_instruction_text_cut_v1(version, text_address, len, limit, out)
                },
                expected,
            );
            assert_eq!(end, 83);
        }
        let invalid = [255_u8, 254];
        assert_eq!(
            unsafe {
                prodex_launch_instruction_text_cut_v1(
                    1,
                    invalid.as_ptr() as usize as u64,
                    2,
                    1,
                    end_ptr,
                )
            },
            1,
        );
        assert_eq!(end, 83);
    }
}
