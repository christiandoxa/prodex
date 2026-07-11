use std::hint::black_box;
use std::time::Duration;

const ALLOCATION_COUNT: usize = 16_384;
const ALLOCATION_BYTES: usize = 16 * 1024;

fn resident_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmRSS:")?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()
            })
        })
        .unwrap_or(0)
}

fn main() {
    let before_kib = resident_kib();
    let allocations = (0..ALLOCATION_COUNT)
        .map(|index| {
            let mut bytes = vec![0_u8; ALLOCATION_BYTES].into_boxed_slice();
            for offset in (0..bytes.len()).step_by(4096) {
                bytes[offset] = index as u8;
            }
            bytes
        })
        .collect::<Vec<_>>();
    black_box(&allocations);
    let peak_kib = resident_kib();
    drop(allocations);
    std::thread::sleep(Duration::from_millis(25));
    let after_drop_kib = resident_kib();
    let trim_reported_reclaim = prodex::bench_support::runtime_allocator_trim_for_benchmark();
    std::thread::sleep(Duration::from_millis(25));
    let after_trim_kib = resident_kib();

    println!(
        "{}",
        serde_json::json!({
            "before_kib": before_kib,
            "peak_kib": peak_kib,
            "after_drop_kib": after_drop_kib,
            "after_trim_kib": after_trim_kib,
            "trim_reported_reclaim": trim_reported_reclaim,
            "trim_reclaimed_kib": after_drop_kib.saturating_sub(after_trim_kib),
            "allocation_count": ALLOCATION_COUNT,
            "allocation_bytes": ALLOCATION_BYTES,
        })
    );
}
