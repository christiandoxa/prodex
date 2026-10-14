#!/usr/bin/env python3
"""Stable release dependency-cache fingerprint excluding workspace version bumps.

Cargo still checks all native fingerprints on restore. Swatinem/rust-cache only
retains dependency crates, not workspace production crates.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import tomllib


def external_dependencies_fingerprint(lockfile: bytes) -> str:
    try:
        data = tomllib.loads(lockfile.decode("utf-8"))
        packages = data["package"]
    except (KeyError, ValueError, UnicodeDecodeError, TypeError) as exc:
        raise ValueError("invalid Cargo.lock content") from exc

    external = []
    for package in packages:
        if "source" not in package:
            continue
        identity = tuple(package.get(field, "") for field in (
            "name", "version", "source", "checksum",
        ))
        if not all(isinstance(value, str) for value in identity):
            raise ValueError("invalid external Cargo package identity")
        if not identity[0] or not identity[1] or not identity[2]:
            raise ValueError("incomplete external Cargo package identity")
        if identity[2].startswith("registry+") and len(identity[3]) != 64:
            raise ValueError("registry package is missing a SHA256 checksum")
        external.append(identity)
    if not external:
        raise ValueError("Cargo.lock contains no external dependencies")
    canonical = json.dumps(sorted(external), ensure_ascii=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lockfile", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    fingerprint = external_dependencies_fingerprint(args.lockfile.read_bytes())
    if not os.environ.get("GITHUB_ACTIONS"):
        parser.error("release cache fingerprint is restricted to GitHub Actions")
    with args.output.open("a", encoding="utf-8") as stream:
        stream.write(f"external_sha256={fingerprint}\n")
    print("verified external Cargo dependency cache key:", fingerprint)


if __name__ == "__main__":
    main()
