#!/usr/bin/env bash
# Hosted Ubuntu already ships versioned LLVM binaries; avoid apt metadata I/O
# just to install unversioned symlinks. All archive ABI checks still run later.
set -euo pipefail
if [ "$#" -gt 1 ]; then
  echo "usage: ensure-llvm-archive-tools.sh [llvm-root]" >&2
  exit 2
fi
if command -v llvm-nm >/dev/null && command -v llvm-ar >/dev/null && command -v llvm-lib >/dev/null; then
  echo "LLVM archive tools already available on PATH"
  exit 0
fi
for directory in "${1:-/usr/lib}"/llvm-*/bin; do
  if [ -x "${directory}/llvm-nm" ] && [ -x "${directory}/llvm-ar" ] && [ -x "${directory}/llvm-lib" ]; then
    # Use one coherent installation, not unrelated tools from mixed versions.
    if [ -z "${GITHUB_PATH:-}" ]; then
      echo "GITHUB_PATH is required to expose the discovered LLVM directory" >&2
      exit 2
    fi
    printf '%s\n' "${directory}" >> "${GITHUB_PATH}"
    echo "Reusing installed LLVM archive tools: ${directory}"
    exit 0
  fi
done
sudo apt-get update
sudo apt-get install -y llvm
command -v llvm-nm
command -v llvm-ar
command -v llvm-lib
