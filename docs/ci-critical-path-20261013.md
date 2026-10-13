# CI and standalone release critical path: 2026-10-13

This note documents measured bottlenecks and the preserving-the-gates workflow
change. The durations below are observed job/step times from GitHub Actions;
they are **not** claimed improvements from the change.

## Baseline evidence

| Run | Gate | Observed bottleneck |
| --- | --- | --- |
| [38348114868](https://github.com/christiandoxa/prodex/actions/runs/38348114868) | Push CI, success | Real Mojo / parity: 24.3 min, including 10.5 min remaining parity tests, 5.5 min all-features Clippy, and 5.2 min independent package parity |
| [38348115030](https://github.com/christiandoxa/prodex/actions/runs/38348115030) | Full CI, success | Real Mojo / parity: 28.9 min, including 12.3 min remaining parity, 7.0 min all-features Clippy, and 6.4 min independent parity |
| [38351207430](https://github.com/christiandoxa/prodex/actions/runs/38351207430) | 0.437.9 release, success | 29.4 min wall time; x86_64 macOS binary build: 18.4 min (16.4 min in the actual build step), following the slowest Mojo archive: 7.2 min |
| [38337354865](https://github.com/christiandoxa/prodex/actions/runs/38337354865) | Earlier release, success | x86_64 macOS binary build: 23.8 min, of which 20.8 min was the build step |
| [38200709046](https://github.com/christiandoxa/prodex/actions/runs/38200709046) | Earlier release, success | Verify-CI gate spent 30.0 min waiting before any Mojo archive could start; x86_64 macOS build then took 16.5 min |

The push CI spent about 24–29 minutes on a job whose independent Clippy,
native linked-binary smoke, four package parity tests, and ten remaining
parity tests ran sequentially. The parity steps launched separate Cargo
processes concurrently in one working tree, so Cargo build-cache locking
also constrained build parallelism.

The standalone release has a separate global archive barrier: the six target
Mojo archives (4–7 min each in parallel) depended on CI verification even
though compiling a source-pinned archive does not require CI success.
Only **binary linking, attestations, and publishing** must wait for both gates.
The other remaining critical path is the macOS Intel production build with
thin LTO; benchmark it rather than weakening release optimization flags.

## Changes and invariants

1. CI reuses the **already compiled, strict, pinned and same-SHA Linux Mojo
   archive** produced by mojo-runtime-linux-archive in the all-features Clippy +
   binary smoke job and the new parity matrix. The parity jobs no longer
   recompile that archive using an independent Mojo compiler.
2. Fourteen parity test commands remain, in three independent runner
   partitions: **4 package, 6 provider/runtime, and 4 app/core**. All retain
   Cargo locked mode, required Mojo, test-thread serialization, and child-process
   failure propagation. The original real-mojo job name remains available
   for existing status checks; CI telemetry waits for the new matrix too.
3. Release Mojo archive builds may start alongside CI verification, after
   checking the exact dispatch SHA and Cargo version against the checkout.
   Each binary build **still depends on both verify-ci and every strict
   target Mojo archive**. No binary can be published by this path if the
   CI gate fails or an archive is missing.
4. Existing full test suites, OS/architecture coverage, native binary smoke,
   checksum validation, six-platform attestations, malware scanning, SBOM,
   installer checks, and provenance constraints remain in place.

## Validation and follow-up measurements

The workflow DAG and 14/14 command inventory are checked by
scripts/ci/ci-pipeline-performance.test.mjs and
scripts/ci/full-rust-test.test.mjs.

After this commit, compare the first successful heavy CI push to
38348114868/38348115030 using **wall time**, runner minutes, the longest
parity partition, and any new cache misses. The matrix consumes more parallel
runners to reduce elapsed time; it is not necessarily a reduction in billed
runner-minutes. Compare subsequent standalone releases to 38351207430 and
watch for Intel macOS builder variance. No target duration is guaranteed
before those remote measurements complete.

Do not remove upstream integration, supply-chain, malware, cross-platform,
or provenance gates merely to make a duration number smaller.


## Measured post-optimization CI and Windows follow-up

[Successful push CI 38360757092](https://github.com/christiandoxa/prodex/actions/runs/38360757092)
on commit 44b54131c completed in **18 min 27 sec** (07:07:55 to 07:26:22
UTC), versus **37 min 02 sec** for [successful push CI 38348114868](https://github.com/christiandoxa/prodex/actions/runs/38348114868).
The environments and cache state differ, so this is an observed comparison,
not a controlled benchmark. The strict real-Mojo lint/binary-smoke job
completed in 1.5 min; its package, provider, and app/core parity partitions
completed in about 1.4, 2.3, and 4.0 min.

That run identified the **new critical path**: Windows prodex-app remaining
library tests. The runner executed **362 tests serially in 531.09 seconds**,
with a total remaining-library job duration of more than 11 minutes; sccache
reported 37 hits and one miss (97% cache hit rate). The problem is therefore
test execution serialization, not primarily dependency compilation.

The Windows-only remainder is now partitioned into four mutually exclusive
runner jobs: continuation/recovery tests, selection/admission tests,
runtime-foundation tests, and the true remainder. Existing targeted Windows
partitions are unchanged. The selection remainder uses a broad libtest match
with explicit skip filters for tests already owned by an earlier Windows
partition. Standard Windows global exclusions, serial execution within each
partition, zero-test rejection, and one cache writer are unchanged.

The hermetic regression tests verify all matrix entries and filter/subfilter
ownership. Against an available local production test binary, **388
representative Linux remainder test names were assigned exactly once**:
66 continuation, 89 selection/admission, 90 runtime-foundation and 143
residual. The actual Windows collection is OS-conditioned (362 in the
measured run), so the next successful Windows CI is authoritative for timing
and platform-specific completeness.

In particular, do not replace the serial Windows suites with a single
multithreaded process: some environment-sensitive tests deliberately require
isolation. Independent GitHub-hosted runners preserve that property.

## Release Intel macOS cross-build hypothesis

The most expensive stable-release build on 0.437.9 was x86_64 macOS on
macos-15-intel: the binary compile itself took 16.4 minutes (20.8 minutes on
another release), compared with about 5.4 minutes for aarch64 macOS on an
Apple Silicon runner. Apple Silicon compilers support the x86_64-apple-darwin
target, but native runtime smoke requires Rosetta. Production releases must
not silently replace a tested Intel native build with an unproven cross build.

The opt-in release-macos-cross-calibration workflow checks out a pinned source
SHA, independently verifies that a successful push CI for that SHA produced
the matching strict x86_64 Mojo archive, and builds the **identical Thin-LTO
release target** on macos-14 (Apple Silicon). It verifies Mach-O x86_64
architecture and requires an actual Rosetta-executed prodex runtime diagnostic.
It publishes neither a binary nor a release. A cross-build that fails on
compiler, linker, architecture, or runtime tests is a rejected experiment,
not permission to weaken release smoke tests.

The actual [cross-build calibration run 38364094252](https://github.com/christiandoxa/prodex/actions/runs/38364094252)
**succeeded** on the pinned 44b54131c source and successful same-SHA
38360757092 CI archive. Full release-profile x86_64 compilation took
**404 seconds (6 min 44 sec)** on macos-14 Apple Silicon and produced a
verified x86_64 Mach-O executable. The actual binary passed version, help
and native Mojo diagnostics **executed under Rosetta**. The preceding
0.437.9 x86_64 native Intel release compilation took **16 min 24 sec**.
This measured build-step improvement is about **59%**, but it is not yet
a controlled comparison of complete standalone release runtimes.

The release matrix therefore switches **only** x86_64-apple-darwin
to the proven macos-14 cross-host. It installs Rosetta if needed, asserts
an actual x86_64 Mach-O target, runs version smoke under Rosetta, and
retains all existing compiled-in Mojo diagnostics and clean-machine smoke.
Its Rust cache key is namespaced to the new cross-host; all other target
caches retain their previous keys. CI verification, six-target compilation
and provenance, checksum, SBOM, malware scan, and publication gates remain
unchanged. Compare the first successful official release after this patch;
it may differ from the isolated calibration due to cold cache and runner
variance.
