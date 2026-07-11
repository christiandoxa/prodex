use std::hint::black_box;
use std::time::Instant;

use argon2::{Algorithm, Argon2, Params, Version};
use prodex_profile_export::{
    PROFILE_EXPORT_ARGON2_ITERATIONS, PROFILE_EXPORT_ARGON2_MEMORY_KIB,
    PROFILE_EXPORT_ARGON2_PARALLELISM,
};
use zeroize::Zeroizing;

fn argument(index: usize, default: u32) -> Result<u32, String> {
    std::env::args()
        .nth(index)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| format!("argument {index} must be an unsigned integer"))
        })
        .unwrap_or(Ok(default))
}

fn main() -> Result<(), String> {
    let memory_kib = argument(1, PROFILE_EXPORT_ARGON2_MEMORY_KIB)?;
    let iterations = argument(2, PROFILE_EXPORT_ARGON2_ITERATIONS)?;
    let parallelism = argument(3, PROFILE_EXPORT_ARGON2_PARALLELISM)?;
    let samples = argument(4, 5)?;
    if !(1..=100).contains(&samples) {
        return Err("samples must be in 1..=100".to_string());
    }
    let params = Params::new(memory_kib, iterations, parallelism, Some(32))
        .map_err(|_| "invalid Argon2id parameters".to_string())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let password = Zeroizing::new(b"prodex-argon2id-benchmark-password".to_vec());
    let salt = b"prodex-benchmark";
    let mut elapsed_ms = Vec::with_capacity(samples as usize);

    for sample in 1..=samples {
        let mut key = Zeroizing::new([0_u8; 32]);
        let started = Instant::now();
        argon2
            .hash_password_into(black_box(&password), black_box(salt), &mut key[..])
            .map_err(|_| "Argon2id derivation failed".to_string())?;
        let millis = started.elapsed().as_secs_f64() * 1_000.0;
        black_box(&key);
        elapsed_ms.push(millis);
        println!("sample={sample} elapsed_ms={millis:.3}");
    }

    elapsed_ms.sort_by(f64::total_cmp);
    let mean = elapsed_ms.iter().sum::<f64>() / f64::from(samples);
    println!(
        "argon2id version=19 memory_kib={memory_kib} iterations={iterations} parallelism={parallelism} samples={samples} min_ms={:.3} median_ms={:.3} mean_ms={mean:.3} max_ms={:.3}",
        elapsed_ms[0],
        elapsed_ms[elapsed_ms.len() / 2],
        elapsed_ms[elapsed_ms.len() - 1],
    );
    Ok(())
}
