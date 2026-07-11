# ADR 0712: Runtime Policy Path Exact Boundary

## Status

Accepted.

## Context

Runtime policy path values, including `runtime.log_dir`, are filesystem paths.
The shared path resolver previously trimmed configured values before resolving
relative paths under the Prodex root. That silently changed valid filesystem
names with leading or trailing spaces.

## Decision

Preserve non-blank runtime policy path values exactly. Validation and the shared
resolver still reject blank-only values, but the resolver no longer trims before
building the `PathBuf`.

## Consequences

Operators get the exact filesystem paths configured in `policy.toml`.
Accidental padding is visible in the resolved path instead of hidden by policy
loading.
