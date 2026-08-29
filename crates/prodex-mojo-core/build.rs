use std::{
    env,
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

fn main() {
    emit_cargo_directives();
    let sources = selected_sources();
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    emit_source_rerun_directives(&sources, &manifest_dir);
    let strict = mojo_required();
    if sources.is_empty() {
        if strict {
            panic!(
                "PRODEX_MOJO_REQUIRED is set but no Mojo subsystem feature is enabled; enable a prodex-mojo-core Mojo feature"
            );
        }
        return;
    }
    build_mojo_archive(sources, manifest_dir, strict);
}

fn emit_cargo_directives() {
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_REQUIRED");
    println!("cargo:rerun-if-env-changed=AR");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_TARGET");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_TARGET_CPU");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_ARCHIVE");
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO_VERSION");
    println!("cargo:rustc-check-cfg=cfg(prodex_mojo_active)");
    println!("cargo:rustc-check-cfg=cfg(prodex_mojo_required)");
}

fn emit_source_rerun_directives(sources: &[&str], manifest_dir: &Path) {
    for source in sources {
        println!(
            "cargo:rerun-if-changed={}",
            manifest_dir.join(source).display()
        );
    }
    if sources.iter().any(|source| source.contains("rich_")) {
        for source in [
            "../../mojo/prodex_core/rich_types.mojo",
            "../../mojo/prodex_core/rich_text.mojo",
        ] {
            println!(
                "cargo:rerun-if-changed={}",
                manifest_dir.join(source).display()
            );
        }
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../mojo/prodex_core/runtime_math.mojo")
            .display()
    );
}

fn build_mojo_archive(sources: Vec<&'static str>, manifest_dir: PathBuf, strict: bool) {
    if strict {
        println!("cargo:rustc-cfg=prodex_mojo_required");
    }
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mojo_override = env::var_os("PRODEX_MOJO");
    let mojo = mojo_override
        .clone()
        .unwrap_or_else(|| OsString::from("mojo"));
    let target = env::var("PRODEX_MOJO_TARGET")
        .or_else(|_| env::var("TARGET"))
        .unwrap_or_else(|_| "x86_64-unknown-linux-gnu".to_string());
    let ar = env::var_os("AR").unwrap_or_else(|| {
        if target.ends_with("-msvc") {
            OsString::from("llvm-lib")
        } else {
            OsString::from("ar")
        }
    });
    let target_cpu = env::var("PRODEX_MOJO_TARGET_CPU")
        .unwrap_or_else(|_| default_target_cpu(&target).to_string());
    let archive = out_dir.join(archive_file_name(&target));
    let expected_version = env::var("PRODEX_MOJO_VERSION").ok();
    if let Some(version) = expected_version.as_deref() {
        if version.is_empty() {
            panic!("PRODEX_MOJO_VERSION must not be empty");
        }
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
            &target,
        );
        return;
    }

    if let Some(version) = expected_version.as_deref() {
        verify_mojo_version(&mojo, version);
    }

    let mut objects = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let source = manifest_dir.join(source);
        let object = out_dir.join(format!("prodex_mojo_core_{index}.o"));
        compile_mojo_source(&mojo, &target, &target_cpu, &source, &object);
        objects.push(object);
    }

    archive_objects(&ar, &archive, &objects, &target);
    emit_link(&out_dir, &target);
}

fn verify_mojo_version(mojo: &OsString, expected: &str) {
    let output = Command::new(mojo).arg("--version").output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            panic!(
                "Mojo compiler was required but not found on PATH; install Mojo or set PRODEX_MOJO"
            )
        }
        Ok(output) => panic!("Mojo compiler version check failed ({})", output.status),
        Err(error) => panic!(
            "failed to run Mojo compiler `{}`: {error}; fix PRODEX_MOJO or disable Mojo features",
            mojo.to_string_lossy()
        ),
    };
    let actual = String::from_utf8_lossy(&output.stdout);
    if !actual
        .trim()
        .strip_prefix("Mojo ")
        .is_some_and(|version| version == expected || version.starts_with(&format!("{expected} ")))
    {
        panic!(
            "unexpected Mojo compiler version: expected {expected}, got {}",
            actual.trim()
        );
    }
}

fn link_prebuilt_archive(
    prebuilt_archive: PathBuf,
    manifest_dir: &Path,
    archive: &Path,
    out_dir: &Path,
    target: &str,
) {
    let prebuilt_archive = resolve_archive_path(prebuilt_archive, manifest_dir);
    if !prebuilt_archive.is_file() {
        panic!(
            "PRODEX_MOJO_ARCHIVE was required but does not exist: {}",
            prebuilt_archive.display()
        );
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
    emit_link(out_dir, target);
}

fn compile_mojo_source(
    mojo: &OsString,
    target: &str,
    target_cpu: &str,
    source: &Path,
    object: &Path,
) {
    let status = Command::new(mojo)
        .arg("build")
        .arg(source)
        .args(["--target-triple", target, "--target-cpu", target_cpu])
        .args(["--emit", "object", "--optimization-level=3", "-o"])
        .arg(object)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            panic!(
                "Mojo compiler was required but not found on PATH; install Mojo or set PRODEX_MOJO"
            );
        }
        Ok(status) => panic_on_failure("Mojo compiler", status, source),
        Err(error) => panic!(
            "failed to run Mojo compiler `{}`: {error}; fix PRODEX_MOJO or disable Mojo features",
            mojo.to_string_lossy()
        ),
    }
}

fn archive_objects(ar: &OsString, archive: &Path, objects: &[PathBuf], target: &str) {
    let mut command = Command::new(ar);
    if target.ends_with("-msvc") {
        command.arg(format!("/out:{}", archive.display()));
        command.args(objects);
    } else {
        command.args(["crus"]).arg(archive).args(objects);
    }
    let status = command.status();
    match status {
        Ok(status) if status.success() => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            panic!("static archiver was required but not found on PATH");
        }
        Ok(status) => panic_on_failure("static archiver", status, archive),
        Err(error) => panic!(
            "failed to run static archiver `{}`: {error}; fix AR or disable Mojo features",
            ar.to_string_lossy()
        ),
    }
}

fn emit_link(out_dir: &Path, _target: &str) {
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=prodex_mojo_core");
    println!("cargo:rustc-cfg=prodex_mojo_active");
}

fn archive_file_name(target: &str) -> &'static str {
    if target.ends_with("-msvc") {
        "prodex_mojo_core.lib"
    } else {
        "libprodex_mojo_core.a"
    }
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
        sources.push("../../mojo/prodex_core/quota_pressure.mojo");
        sources.push("../../mojo/prodex_core/runtime_auto_redeem.mojo");
        sources.push("../../mojo/prodex_core/profile_schedule.mojo");
        sources.push("../../mojo/prodex_core/candidate_decision.mojo");
        sources.push("../../mojo/prodex_core/smart_context_rehydrate.mojo");
        sources.push("../../mojo/prodex_core/runtime_tuning.mojo");
        sources.push("../../mojo/prodex_core/smart_context.mojo");
        sources.push("../../mojo/prodex_core/policy_validation.mojo");
        sources.push("../../mojo/prodex_core/context.mojo");
        sources.push("../../mojo/prodex_core/context_text.mojo");
    }
    if env::var_os("CARGO_FEATURE_MOJO_RICH").is_some()
        || env::var_os("CARGO_FEATURE_MOJO_CORE").is_some()
        || env::var_os("CARGO_FEATURE_MOJO_RUNTIME").is_some()
    {
        sources.push("../../mojo/prodex_core/rich_abi.mojo");
        sources.push("../../mojo/prodex_core/rich_context_v2.mojo");
        sources.push("../../mojo/prodex_core/rich_route.mojo");
        sources.push("../../mojo/prodex_core/rich_policy.mojo");
        sources.push("../../mojo/prodex_core/rich_fallback.mojo");
        sources.push("../../mojo/prodex_core/rich_plan.mojo");
        sources.push("../../mojo/prodex_core/rich_catalog.mojo");
        sources.push("../../mojo/prodex_core/log_semantics.mojo");
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

fn panic_on_failure(tool: &str, status: ExitStatus, path: &Path) -> ! {
    panic!("{tool} failed for {} ({status})", path.display());
}
