use std::path::Path;

use super::{ContextBlobNoiseFinding, ContextBlobNoiseKind};

pub(super) fn detect_lockfile_or_vendor_noise_supplement(
    path: Option<&Path>,
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    if let Some(path) = path
        && let Some(detail) = context_lockfile_or_vendor_path_detail_supplement(path)
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: 100,
            detail,
        });
    }

    let cargo_packages = lines
        .iter()
        .filter(|line| line.trim() == "[[package]]")
        .count();
    let cargo_checksums = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("checksum = "))
        .count();
    if cargo_packages >= 4 && cargo_checksums >= 2 {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 95,
            detail: format!("cargo_lock_packages={cargo_packages}"),
        });
    }

    let lower = input.to_ascii_lowercase();
    if lower.contains("\"lockfileversion\"")
        && (lower.contains("\"packages\"") || lower.contains("\"dependencies\""))
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 95,
            detail: "npm_lockfile_json".to_string(),
        });
    }

    if lower.contains("# yarn lockfile")
        || lower.contains("pnpm-lock.yaml")
        || lower.contains("lockfileversion:")
            && (lower.contains("importers:") || lower.contains("packages:"))
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 90,
            detail: "package_manager_lockfile".to_string(),
        });
    }

    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let mut vendor_path_lines = 0usize;
    let mut first_vendor_line = None;
    for (index, line) in lines.iter().enumerate() {
        if context_line_has_vendor_path_supplement(line) {
            vendor_path_lines += 1;
            first_vendor_line.get_or_insert(index + 1);
        }
    }
    if vendor_path_lines >= 8
        && vendor_path_lines.saturating_mul(100) >= non_empty_lines.max(1) * 40
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: first_vendor_line,
            bytes: input.len(),
            score: 85,
            detail: format!("vendor_path_lines={vendor_path_lines}"),
        });
    }

    None
}

fn context_lockfile_or_vendor_path_detail_supplement(path: &Path) -> Option<String> {
    let normalized = path.display().to_string().replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    let file_name = lower.rsplit('/').next().unwrap_or(lower.as_str());
    if matches!(
        file_name,
        "cargo.lock"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lock"
            | "bun.lockb"
            | "poetry.lock"
            | "pipfile.lock"
            | "gemfile.lock"
            | "composer.lock"
            | "go.sum"
    ) {
        return Some(format!("lockfile_path={normalized}"));
    }

    [
        "/node_modules/",
        "/vendor/",
        "/third_party/",
        "/.cargo/registry/",
        "/.pnpm/",
        "/.yarn/cache/",
    ]
    .into_iter()
    .find(|marker| {
        lower.contains(marker) || lower.trim_start_matches("./").starts_with(&marker[1..])
    })
    .map(|_| format!("vendor_path={normalized}"))
}

fn context_line_has_vendor_path_supplement(line: &str) -> bool {
    let lower = line.replace('\\', "/").to_ascii_lowercase();
    [
        "/node_modules/",
        "node_modules/",
        "/vendor/",
        "vendor/",
        "/third_party/",
        "third_party/",
        "/.cargo/registry/",
        ".cargo/registry/",
        "/.pnpm/",
        ".pnpm/",
        "/.yarn/cache/",
        ".yarn/cache/",
    ]
    .into_iter()
    .any(|marker| lower.contains(marker))
}
