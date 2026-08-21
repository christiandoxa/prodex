use std::{
    env,
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

fn main() {
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_REQUIRED");
    println!("cargo:rerun-if-env-changed=AR");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_TARGET");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_TARGET_CPU");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_ARCHIVE");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_VERSION");
    println!("cargo:rustc-check-cfg=cfg(prodex_mojo_fallback)");
    println!("cargo:rustc-check-cfg=cfg(prodex_mojo_active)");
    println!("cargo:rustc-check-cfg=cfg(prodex_mojo_required)");

    let sources = selected_sources();
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    for source in &sources {
        println!(
            "cargo:rerun-if-changed={}",
            manifest_dir.join(source).display()
        );
    }

    let required = mojo_required();
    if required {
        println!("cargo:rustc-cfg=prodex_mojo_required");
    }
    if sources.is_empty() {
        if required {
            panic!(
                "PRODEX_MOJO_REQUIRED is set but no Mojo subsystem feature is enabled; enable mojo-quota, mojo-runtime, or mojo-routing"
            );
        }
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let archive = out_dir.join("libprodex_mojo_core.a");
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mojo_override = env::var_os("PRODEX_MOJO");
    let mojo = mojo_override
        .clone()
        .unwrap_or_else(|| OsString::from("mojo"));
    let ar_override = env::var_os("AR");
    let ar = ar_override.clone().unwrap_or_else(|| OsString::from("ar"));
    let target = env::var("PRODEX_MOJO_TARGET")
        .or_else(|_| env::var("TARGET"))
        .unwrap_or_else(|_| "x86_64-unknown-linux-gnu".to_string());
    let target_cpu = env::var("PRODEX_MOJO_TARGET_CPU")
        .unwrap_or_else(|_| default_target_cpu(&target).to_string());
    if let Ok(version) = env::var("PRODEX_MOJO_VERSION") {
        println!("cargo:rustc-env=PRODEX_MOJO_VERSION={version}");
    }

    if archive.exists() {
        fs::remove_file(&archive).unwrap_or_else(|error| {
            panic!(
                "failed to remove stale Mojo archive {}: {error}",
                archive.display()
            )
        });
    }

    if let Some(prebuilt_archive) = env::var_os("PRODEX_MOJO_ARCHIVE") {
        link_prebuilt_archive(
            PathBuf::from(prebuilt_archive),
            &manifest_dir,
            &archive,
            &out_dir,
            required,
        );
        return;
    }

    for (index, source) in sources.iter().enumerate() {
        let source = manifest_dir.join(source);
        let object = out_dir.join(format!("prodex_mojo_core_{index}.o"));
        if !compile_mojo_source(
            &mojo,
            mojo_override.is_some(),
            &target,
            &target_cpu,
            &source,
            &object,
            required,
        ) || !archive_object(&ar, ar_override.is_some(), &archive, &object, required)
        {
            return;
        }
    }

    emit_link(&out_dir);
}

fn link_prebuilt_archive(
    prebuilt_archive: PathBuf,
    manifest_dir: &Path,
    archive: &Path,
    out_dir: &Path,
    required: bool,
) {
    let prebuilt_archive = resolve_archive_path(prebuilt_archive, manifest_dir);
    if !prebuilt_archive.is_file() {
        if required {
            panic!(
                "PRODEX_MOJO_ARCHIVE was required but does not exist: {}",
                prebuilt_archive.display()
            );
        }
        use_rust_fallback("configured Mojo archive does not exist");
        return;
    }
    fs::copy(&prebuilt_archive, archive).unwrap_or_else(|error| {
        panic!(
            "failed to copy prebuilt Mojo archive {}: {error}",
            prebuilt_archive.display()
        )
    });
    println!(
        "cargo:warning=prodex-mojo-core linking prebuilt target archive {}",
        prebuilt_archive.display()
    );
    emit_link(out_dir);
}

fn compile_mojo_source(
    mojo: &OsString,
    configured: bool,
    target: &str,
    target_cpu: &str,
    source: &Path,
    object: &Path,
    required: bool,
) -> bool {
    let status = Command::new(mojo)
        .arg("build")
        .arg(source)
        .args(["--target-triple", target, "--target-cpu", target_cpu])
        .args(["--emit", "object", "--optimization-level=3", "-o"])
        .arg(object)
        .status();
    match status {
        Ok(status) if status.success() => true,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if required {
                panic!(
                    "Mojo compiler was required but not found on PATH; install Mojo or set PRODEX_MOJO"
                );
            }
            if configured {
                panic!(
                    "configured Mojo compiler `{}` was not found; fix PRODEX_MOJO or disable Mojo features",
                    mojo.to_string_lossy()
                );
            }
            use_rust_fallback("Mojo compiler not found on PATH");
            false
        }
        Ok(status) => panic_on_failure("Mojo compiler", status, source),
        Err(error) => panic!(
            "failed to run Mojo compiler `{}`: {error}; fix PRODEX_MOJO or disable Mojo features",
            mojo.to_string_lossy()
        ),
    }
}

fn archive_object(
    ar: &OsString,
    configured: bool,
    archive: &Path,
    object: &Path,
    required: bool,
) -> bool {
    let status = Command::new(ar)
        .args(["crus"])
        .arg(archive)
        .arg(object)
        .status();
    match status {
        Ok(status) if status.success() => true,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if required {
                panic!("static archiver was required but not found on PATH");
            }
            if configured {
                panic!(
                    "configured static archiver `{}` was not found; fix AR or disable Mojo features",
                    ar.to_string_lossy()
                );
            }
            use_rust_fallback("static archiver not found on PATH");
            false
        }
        Ok(status) => panic_on_failure("static archiver", status, archive),
        Err(error) => panic!(
            "failed to run static archiver `{}`: {error}; fix AR or disable Mojo features",
            ar.to_string_lossy()
        ),
    }
}

fn emit_link(out_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=prodex_mojo_core");
    println!("cargo:rustc-cfg=prodex_mojo_active");
}

fn selected_sources() -> Vec<&'static str> {
    let mut sources = Vec::new();
    if env::var_os("CARGO_FEATURE_MOJO_QUOTA").is_some()
        || env::var_os("CARGO_FEATURE_MOJO_CORE").is_some()
    {
        sources.push("../../mojo/prodex_core/quota.mojo");
    }
    if env::var_os("CARGO_FEATURE_MOJO_RUNTIME").is_some()
        || env::var_os("CARGO_FEATURE_MOJO_CORE").is_some()
    {
        sources.push("../../mojo/prodex_core/runtime_quota.mojo");
        sources.push("../../mojo/prodex_core/smart_context.mojo");
    }
    if env::var_os("CARGO_FEATURE_MOJO_ROUTING").is_some()
        || env::var_os("CARGO_FEATURE_MOJO_CORE").is_some()
    {
        sources.push("../../mojo/prodex_core/routing_score.mojo");
    }
    if env::var_os("CARGO_FEATURE_MOJO_PROVIDER_CONSTRAINTS").is_some()
        || env::var_os("CARGO_FEATURE_MOJO_CORE").is_some()
    {
        sources.push("../../mojo/prodex_core/provider_constraints.mojo");
    }
    sources
}

fn mojo_required() -> bool {
    env::var("PRODEX_MOJO_REQUIRED")
        .map(|value| !matches!(value.as_str(), "" | "0" | "false" | "no"))
        .unwrap_or(false)
}

fn default_target_cpu(target: &str) -> &'static str {
    if target.starts_with("x86_64-") {
        "x86-64"
    } else {
        "generic"
    }
}

fn resolve_archive_path(path: PathBuf, manifest_dir: &Path) -> PathBuf {
    if path.is_absolute() || path.is_file() {
        path
    } else {
        manifest_dir.join("../..").join(path)
    }
}

fn use_rust_fallback(reason: &str) {
    println!("cargo:warning=prodex-mojo-core using Rust fallback: {reason}");
    println!("cargo:rustc-cfg=prodex_mojo_fallback");
}

fn panic_on_failure(tool: &str, status: ExitStatus, path: &Path) -> ! {
    panic!("{tool} failed for {} ({status})", path.display());
}
