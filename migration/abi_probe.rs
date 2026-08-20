#[link(name = "prodex_abi_probe", kind = "static")]
extern "C" {
    fn prodex_abi_probe_sum_u64(
        values: *const u64,
        length: i64,
        output: *mut u64,
    ) -> i64;
}

fn main() {
    let values = [1_u64, 2, u64::MAX - 3];
    let mut output = 0_u64;
    let status = unsafe {
        prodex_abi_probe_sum_u64(values.as_ptr(), values.len() as i64, &mut output)
    };
    assert_eq!(status, 0);
    assert_eq!(output, u64::MAX);

    let mut empty_output = u64::MAX;
    let status = unsafe { prodex_abi_probe_sum_u64(values.as_ptr(), 0, &mut empty_output) };
    assert_eq!(status, 0);
    assert_eq!(empty_output, 0);
}
