use std::{
    env,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

fn main() {
    println!("cargo:rerun-if-env-changed=PRODEX_MOJO");
    println!("cargo:rerun-if-env-changed=AR");
    println!("cargo:rustc-check-cfg=cfg(prodex_mojo_fallback)");

    if env::var_os("CARGO_FEATURE_MOJO").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = manifest_dir.join("../../mojo/prodex_core/quota.mojo");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out_dir.join("prodex_quota_mojo.o");
    let archive = out_dir.join("libprodex_quota_mojo.a");
    let mojo_override = env::var_os("PRODEX_MOJO");
    let mojo = mojo_override
        .clone()
        .unwrap_or_else(|| OsString::from("mojo"));
    let ar_override = env::var_os("AR");
    let ar = ar_override.clone().unwrap_or_else(|| OsString::from("ar"));

    println!("cargo:rerun-if-changed={}", source.display());

    let status = Command::new(&mojo)
        .arg("build")
        .arg(&source)
        .args(["--emit", "object", "--optimization-level=3", "-o"])
        .arg(&object)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Err(error) if error.kind() == ErrorKind::NotFound && mojo_override.is_none() => {
            use_rust_fallback("Mojo compiler not found on PATH");
            return;
        }
        Ok(status) => panic_on_failure("Mojo compiler", status, &source),
        Err(error) => panic!(
            "failed to run Mojo compiler `{}`: {error}; fix PRODEX_MOJO or disable feature `mojo`",
            mojo.to_string_lossy()
        ),
    }

    let status = Command::new(&ar)
        .args(["crus"])
        .arg(&archive)
        .arg(&object)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Err(error) if error.kind() == ErrorKind::NotFound && ar_override.is_none() => {
            use_rust_fallback("static archiver not found on PATH");
            return;
        }
        Ok(status) => panic_on_failure("static archiver", status, &archive),
        Err(error) => panic!(
            "failed to run static archiver `{}`: {error}; fix AR or disable feature `mojo`",
            ar.to_string_lossy()
        ),
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=prodex_quota_mojo");
}

fn use_rust_fallback(reason: &str) {
    println!("cargo:warning=prodex-quota Mojo feature using Rust fallback: {reason}");
    println!("cargo:rustc-cfg=prodex_mojo_fallback");
}

fn panic_on_failure(tool: &str, status: ExitStatus, path: &Path) -> ! {
    panic!(
        "{tool} failed for {} ({status}); disable feature `mojo`",
        path.display()
    );
}
