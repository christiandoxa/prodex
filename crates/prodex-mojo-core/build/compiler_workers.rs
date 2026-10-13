//! Strict Mojo subprocess compilation with bounded concurrency.
//! Output order is owned by the caller's indexed archive member list.

use std::{
    env,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
};

/// Bound subprocess concurrency independently of the number of Mojo files.
/// Cargo supplies NUM_JOBS for build scripts; an override controls memory use.
pub(super) fn mojo_compile_jobs(source_count: usize) -> usize {
    let available = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let cargo_jobs = env::var("NUM_JOBS").ok();
    let override_jobs = env::var("PRODEX_MOJO_COMPILE_JOBS").ok();
    configured_mojo_jobs(
        source_count,
        cargo_jobs.as_deref(),
        override_jobs.as_deref(),
        available,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

pub(super) fn configured_mojo_jobs(
    source_count: usize,
    cargo_jobs: Option<&str>,
    override_jobs: Option<&str>,
    available: usize,
) -> Result<usize, String> {
    // One Mojo compiler can use substantial memory, especially on CI.
    const MAX_MOJO_JOBS: usize = 8;
    const DEFAULT_MOJO_JOBS: usize = 4;
    if source_count == 0 {
        return Ok(1);
    }
    let default = cargo_jobs
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|jobs| *jobs > 0)
        .unwrap_or(available.max(1))
        .min(DEFAULT_MOJO_JOBS);
    let selected = match override_jobs {
        None => default,
        Some(value) => {
            let jobs = value.parse::<usize>().map_err(|_| {
                format!("PRODEX_MOJO_COMPILE_JOBS must be an integer from 1 to {MAX_MOJO_JOBS}")
            })?;
            if !(1..=MAX_MOJO_JOBS).contains(&jobs) {
                return Err(format!(
                    "PRODEX_MOJO_COMPILE_JOBS must be an integer from 1 to {MAX_MOJO_JOBS}"
                ));
            }
            jobs
        }
    };
    Ok(selected.min(source_count).max(1))
}

/// Multiple child compilers may emit diagnostics, but the archive remains
/// fail-closed: a failed child suppresses subsequent archive/link steps.
pub(super) fn compile_mojo_objects(
    mojo: &OsString,
    target: &str,
    target_cpu: &str,
    work: &[(PathBuf, PathBuf)],
    jobs: usize,
) {
    assert!(jobs >= 1, "Mojo worker count must be positive");
    if jobs == 1 {
        for (source, object) in work {
            if let Err(error) = compile_mojo_source(mojo, target, target_cpu, source, object) {
                panic!("{error}");
            }
        }
        return;
    }
    let cursor = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let errors = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(jobs);
        for _ in 0..jobs {
            handles.push(scope.spawn(|| {
                compile_next_mojo_object(&cursor, &failed, work, mojo, target, target_cpu)
            }));
        }
        handles
            .into_iter()
            .filter_map(|handle| handle.join().expect("Mojo worker thread panicked"))
            .collect::<Vec<_>>()
    });
    if !errors.is_empty() {
        panic!(
            "one or more Mojo translation units failed:\n{}",
            errors.join("\n")
        );
    }
}

/// Each worker claims a stable output index and stops claiming new sources
/// immediately after another worker reports a compile failure.
fn compile_next_mojo_object(
    cursor: &AtomicUsize,
    failed: &AtomicBool,
    work: &[(PathBuf, PathBuf)],
    mojo: &OsString,
    target: &str,
    target_cpu: &str,
) -> Option<String> {
    loop {
        if failed.load(Ordering::Acquire) {
            return None;
        }
        let index = cursor.fetch_add(1, Ordering::Relaxed);
        let (source, object) = work.get(index)?;
        if let Err(error) = compile_mojo_source(mojo, target, target_cpu, source, object) {
            failed.store(true, Ordering::Release);
            return Some(error);
        }
    }
}

fn compile_mojo_source(
    mojo: &OsString,
    target: &str,
    target_cpu: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let output = Command::new(mojo)
        .arg("build")
        .arg(source)
        .args(["-D", "HEAP_BUFFER_BYTES=4096"])
        .args(["--target-triple", target, "--target-cpu", target_cpu])
        .args(["--emit", "object", "--optimization-level=3", "-o"])
        .arg(object)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            if !output.stderr.is_empty() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Err(
            "Mojo compiler was required but not found on PATH; install Mojo or set PRODEX_MOJO"
                .to_string(),
        ),
        Ok(output) => Err(format!(
            "Mojo compiler failed for {} ({}):\n{}{}",
            source.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )),
        Err(error) => Err(format!(
            "failed to run Mojo compiler {} for {}: {error}; fix PRODEX_MOJO",
            mojo.to_string_lossy(),
            source.display()
        )),
    }
}
