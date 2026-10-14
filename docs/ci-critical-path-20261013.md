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


## Parallel Mojo translation-unit compilation (2026-10-13)

The strict Mojo archive compiler in crates/prodex-mojo-core/build.rs previously
called the external Mojo compiler once **serially** for every translation unit.
A complete release-profile Linux archive currently contains 129 separate
compiled objects. This is shared by all CI strict archive targets and six
standalone-release archive targets, so reducing the root compile cost improves
both pipelines.

The build script now runs independent Mojo compilation processes concurrently:
it defaults to at most **4** subprocesses (also bounded by Cargo's NUM_JOBS
and the number of sources). Resource-constrained developers may set
PRODEX_MOJO_COMPILE_JOBS=1; values 1 through 8 are accepted, while malformed
or excessive overrides fail explicitly rather than silently continuing with
an unsafe value. A failure in any compiler aborts archive creation and Rust
linking, after waiting for the remaining in-flight children. Archiving still
uses the original indexed object order and Windows COFF weak-symbol isolation
still runs *after* all compilation, not concurrently with it.

A controlled local Mojo 1.1.0 / Rust 1.99.0 release-profile comparison of the
same 129-source x86_64 Linux archive recorded:

| Compilation | Total Cargo build duration | Archive member count |
| --- | ---: | ---: |
| Serial, 1 Mojo worker | **142 seconds** | 129 |
| Parallel, 4 Mojo workers | **44 seconds** | 129 |

That is a **69% reduction in local build duration**, or about **3.2 times
faster** for this workload. The two resulting archives had identical ordered
member names and identical sets of exported symbols, including all inspected
ABI entry points. Neither archive referenced the forbidden Mojo compiler
runtime. Separate Cargo target directories were used to avoid shared-cache
races, and both tests used the same source, target, release profile, and
pinned toolchains. This result is a local benchmark, not yet a promise of
specific Windows/macOS CI runtime reductions.

The build-script unit tests check worker limits, deterministic object
creation, and fail-closed behavior on a deliberate failing child. A lightweight
Rust test harness is included in the existing CI static-guard fan-out, so
future changes cannot accidentally revert to serial-only compilation or
silently allow a partial archive.

In subsequent GitHub Actions compare the per-platform strict Mojo archive
*build steps* with the old 4-7 minute timings; a new timing log exposes the
worker count and translation-unit compile duration directly. The source must
still pass all existing platform archive, compiled-in runtime, and release
provenance checks before publishing.


## Reuse verified CI Mojo archives during standalone release

The parallel compiler still left expensive duplication: a successful heavy
CI push already builds and verifies strict archives for Linux x86_64, macOS
x86_64, macOS ARM64, and Windows x86_64, and standalone release previously
compiled these **same four archives again** on the same source SHA, target CPU
policy, and Mojo 1.1.0 compiler. Only Linux ARM64 and Windows ARM64 lack CI
platform archive artifacts.

The release archive matrix still prepares all **six** release archives,
but first attempts to reuse the four same-source CI archives. The helper
scripts/ci/reuse_mojo_ci_archive.py requires matching checked-out HEAD,
release dispatch SHA and GitHub workflow SHA; it also verifies the source
CI run's repository, push event, main branch, workflow path and eligible
status, and checks that the named artifact is present, nonexpired and
nonempty. It extracts to a temporary directory and atomically places the
verified payload under the existing release archive path.

If **any condition fails**, including a lightweight CI with no platform
archive, network interruption, missing run, or mismatched SHA, the job runs
the original pinned Mojo compiler installation and strict source compilation.
It never accepts a matching archive solely by filename. Both reused and
freshly built archives still pass the unchanged exported-symbol and
compiler-runtime checks; binary builds continue to require complete
successful CI and the full six-target release archive matrix.

One real integration exercise used successful push CI
[38360757092](https://github.com/christiandoxa/prodex/actions/runs/38360757092)
from source 44b54131c8b91e17f728be7854a16882e6867c80.
The exact-source Linux x86_64 archive was downloaded and passed the required
ABI symbol and forbidden compiler-runtime checks; its SHA-256 was
9507aedd97278644e8be6684c1ca79c3eef3e399d52e1b68ec66cec8ecec63b3.
The same exercise confirmed Linux ARM64 correctly selects strict fresh
compilation because CI does not publish that platform archive.

This removes repeated Mojo toolchain installations and four unnecessary
archive compilations from many standalone releases, reducing billed runner
time. Do **not** claim a specific official standalone release wall-clock
speedup before completing a release on the updated path: Linux ARM64 and
Windows ARM64 still compile, and runner scheduling may dominate wall time.

Hermetic tests cover source/run provenance, canceled CI, expired archives,
interrupted download, and required fresh-build fallback. The CI static
guards execute them for future changes.


## Trust-scoped source-fingerprint cache for CI Mojo archives

After the four-worker compiler improvement, the fully successful push CI
[38370749687](https://github.com/christiandoxa/prodex/actions/runs/38370749687)
completed in about **12 min 9 sec**, compared to **18 min 27 sec** on the
earlier full push CI [38360757092](https://github.com/christiandoxa/prodex/actions/runs/38360757092).
Different cache/runner states mean this is an observed comparison, not a
guaranteed future speedup.

The next repeated expense was compiling all **four** strict Mojo archives
on each heavy push, including pushes that only modify Rust adapters, CI
scripts, or documentation. CI build steps on commit 4b1c9de03 each still
took approximately **3 min 41 sec to 3 min 52 sec**. The assembled archive
depends on the Mojo source tree and build-script code, not on the identity
of the Git commit itself.

On **trusted main-branch push runs only**, the x86_64 Linux, x86_64 Windows,
x86_64 macOS, and aarch64 macOS archive jobs now use an immutable GitHub
cache key with the exact target triple, target CPU, Mojo version, pinned
Rust version, and hash of all Mojo modules, the Mojo build script and
helper modules, workspace Cargo manifests and Cargo.lock. The key does not
have prefix restores; a changed input cannot select a nearby cached
archive. Pull requests, schedules and manual dispatches always compile
from source because they do not qualify for a trusted push cache entry.

With a hit, the job avoids redundant Rust/uv/Mojo toolchain installation
and 129-object Mojo compilation. It still obtains the LLVM archive tools,
verifies exported ABI symbols, rejects forbidden Mojo compiler-runtime
references, and applies the existing MSVC duplicate weak-symbol check
before publishing the ordinary CI archive artifact. With a miss, every
original strict build step executes. The GitHub cache is saved only after
the verification and upload job succeeds. The release-specific same-SHA
archive reuse continues to work independently.

The LLVM setup additionally avoids apt-get update/install when all three
required LLVM tools are already present, while keeping the original
installer if any are unavailable.

This optimization removes redundant **compilation** on repeated unchanged
Mojo inputs; it does not remove tests, audits, supply-chain checks, or
artifact verification. The first cache-key use must populate the cache,
so its push CI can still take the prior 4-minute archive build time.
Measure the next trusted push with unchanged Mojo inputs to confirm
cache-hit behavior and actual CI wall-clock improvement.


## Second Windows continuation partitioning

Successful push CI 38370749687 and 38372395176 showed that the largest
remaining Windows library partition was **66 serial continuation/recovery
tests**, taking close to five minutes including runner setup, while the
other Windows remainder partitions finished sooner. Changing global
libtest thread counts risks races in shared-port and session-affinity tests,
so concurrency remains **across independent GitHub runners only**.

The continuation partition is now divided into HTTP follow-ups,
WebSocket continuation/recovery, and a catch-all continuation group.
Each is run serially with the unchanged Windows global skips and zero-test
match rejection. Targeted HTTP/WebSocket prefixes are excluded from the
catch-all group using explicit libtest --skip filters, so newer continuation
modules automatically stay covered by the catch-all.

A real local prodex-app test-binary enumeration assigned **all 66** of the
previous continuation test names **exactly once**: 25 HTTP follow-ups,
23 WebSocket, 18 other continuation tests. CI's generator tests also
exercise eleven representative path identities, including future new modules.
The Windows matrix increases from 8 to 10 runners on heavy pushes, trading
additional runner setup/billed minutes for lower wall-clock duration while
preserving per-shard serialization. The next completed Windows CI is needed
to confirm actual speedup and environment-specific test coverage.


## Release metadata-aware archive fingerprints and ARM64 reuse

GitHub CI source-fingerprint caching was verified on
[run 38375872402](https://github.com/christiandoxa/prodex/actions/runs/38375872402):
all four archive jobs skipped compilation and preserved their unconditional
ABI checks and artifact uploads. The jobs finished in **17, 18, 22 and
28 seconds**, versus ~4m15s–4m23s on the prior full compile. This
is an observed same-input cache hit, not a promise that every push will hit.

The initial fingerprint unnecessarily included the workspace Cargo.toml and
Cargo.lock. Updating Prodex's package version or unrelated Rust dependencies
can change these files without changing a Mojo translation unit. The CI
fingerprint is now v2: it covers all Mojo source files (including imports),
the complete build script and Rust helper source files, the
prodex-mojo-core feature manifest, the target architecture and CPU, pinned
Rust 1.99.0 and Mojo 1.1.0. Workspace version-only metadata no longer
invalidates the archive, but any meaningful compiler/ABI/source change
does. There are **no prefix restore keys**; a cache miss always uses a
fresh strict source build, and the ABI/forbidden-runtime checks run on
hits and misses alike. The new v2 fingerprint naturally needs to populate
its cache once before the next run can hit.

Standalone release additionally builds Linux ARM64 and Windows ARM64 Mojo
archives; heavy CI currently publishes only the other four targets.
These two remaining archives now have a **separate, immutable and
architecture-scoped release-only cache** with the same compiler/source
fingerprint. The workflow tries exact-SHA provenance-verified CI archive
reuse first; on the ARM64 targets it then tries the immutable main-ref
cache, and if neither succeeds installs the pinned compiler and builds
the archive. The release archive cache is never used for other targets,
PRs or forks, and it is saved only on a fully successful archive job.
All six archives still undergo the same ABI symbol, MSVC weak-symbol,
and forbidden Mojo CompilerRT checks before artifact publication.
The binary build remains behind the successful full-CI and complete
six-platform archive gates. Version changes alone no longer trigger
ARM64 Mojo recompilation when sources and compiler inputs are unchanged.

Expected benefit: eliminate **two redundant ARM64 archive builds** on
subsequent source-identical releases. The first release with these cache keys
must compile to populate them. This reduces release compute and may shorten
the archive barrier; Windows ARM64 Rust binary compilation still
determines a substantial portion of the remaining release critical path.
No measured official release timing is claimed until a new release
actually uses the cache.


## Native Windows ARM64 release benchmark (opt-in; not yet adopted)

Across published standalone releases 38351207430 and 38337354865,
**Windows ARM64 binary compilation on an x86_64 Windows runner**
took 9m42s and 8m54s respectively. This is the remaining largest
production Rust binary-build step after moving Intel macOS compilation
onto Apple Silicon. The release still uses the audited x86_64 runner
until a native alternative proves both faster and equally correct.

GitHub supports the generally available standard windows-11-arm
hosted runner for Windows ARM64, and the current image uses VS2026.
The new opt-in
.github/workflows/release-windows-arm64-calibration.yml
benchmark uses that native host and the pinned 0.437.9 source
and ARM64 Mojo archive from the **same successful official release
workflow**. It checks repository, workflow identity, commit SHA, main
branch, completed-success provenance, and the actual checked-out HEAD.
It builds a real Thin-LTO aarch64-pc-windows-msvc production binary,
validates PE Machine 0xAA64, then runs version/help/runtime-doctor
smokes **natively** with compiled-in Mojo required and no fallback.

The benchmark uploads only a text timing record. It never publishes a
binary, modifies tags or changes the official six-platform release
matrix. The official Windows ARM64 runner must remain unchanged until
the benchmark succeeds in compilation, native ABI/runtime, and measured
critical-path performance. Any failure is a reason to investigate
platform portability, not to disable supply-chain or smoke checks.


## Official Windows ARM64 native release runner (measured)

The opt-in [native ARM64 Windows calibration 38378915483](https://github.com/christiandoxa/prodex/actions/runs/38378915483)
completed **successfully** using the **exact 0.437.9 release source**
and Mojo archive from the successful official release 38351207430.
The binary was a valid PE ARM64 executable (Machine 0xAA64) and
actually executed version/help and compiled-in Mojo diagnostics on
a native windows-11-arm runner. The archive's ABI symbols and absence
of forbidden CompilerRT dependencies were independently checked.

Measured Thin-LTO production ARM64 build duration: **336 seconds
(5m36s)** versus **582 seconds (9m42s)** on the previous official
x86_64 Windows runner for 0.437.9. This is a measured **42% decrease
in that build step** with the same source and release target, though
runner cache/image differences still affect a direct comparison.
A second previous release required 8m54s. The benchmark did not
publish binaries or change release tags.

Official standalone release now uses the verified native ARM64 image
**for only aarch64-pc-windows-msvc**. Its Rust cache namespace is
separated from x86 cross-host objects, native ARM builds use Cargo
directly without assuming an ARM64-compatible sccache wrapper, and
every ARM binary must pass an explicit PE architecture check, native
version smoke, and the full existing compiled-in Mojo/clean-machine
runtime diagnostics (the latter were previously unexecutable on x86
runners). The same six-target matrix and full CI/archive/provenance
publication dependencies remain intact.

The first full standalone release using this runner is still needed
to quantify its overall wall-clock benefit; the previous bottleneck
also included antivirus and other independent release gates. If a
platform-specific smoke check fails, fix its platform behavior rather
than dropping the diagnostic or weakening security.


## Eliminate duplicate nightly Mojo compilation

The scheduled/dispatch Full Rust suite still ran its own strict Linux Mojo
archive compiler despite the exact same binary archive already being cached
by trusted main CI. Its archive-producing job blocks every full-test
partition, so unlike a parallel independent stage, it directly adds to
end-to-end critical-path time. The last two successful full-test runs showed:

| Full test run | Archive job | Compilation step |
| --- | ---: | ---: |
| [38289151967](https://github.com/christiandoxa/prodex/actions/runs/38289151967) | 7.0 minutes | 396 seconds |
| [38149665434](https://github.com/christiandoxa/prodex/actions/runs/38149665434) | 6.6 minutes | 373 seconds |

The nightly Linux archive producer now uses the **exact same immutable
source-fingerprint key** as CI's trusted v2 Linux archive, with the
same archive path inside its own job. This allows scheduled main or
explicit main dispatch to restore the already validated cache. It does
not use the cache for any other source branch. If a cache misses, the
original Rust 1.99.0 / uv / Mojo 1.1.0 toolchain is installed and the
full strict compile executes exactly as before.

After either a hit or a miss, it **unconditionally** checks exported ABI
symbols, the forbidden Mojo compiler-runtime dependency and nonempty
archive bytes before uploading the archive for all full-test consumers.
Those test consumers still download into the same prior layout and
execute every scheduled/full Rust test partition with the existing
error propagation and per-test serialization.

This change removes up to ~6.2–6.6 minutes of redundant compilation
on a matching cache hit. Actual full-workflow wall-clock savings may
be smaller or larger due to runner contention, cache access, and other
independent tests; no optimized nightly run is claimed until measured.
Regression tests require byte-identical cache keys, all mandatory guards
and the original consumer matrix/downstream test execution.


## Shard the nightly unassigned prodex-app remainder

Even after caching the Mojo prerequisite, the scheduled Full Rust suite's
unassigned prodex-app remainder was an independent critical path. It took
**338 seconds** of test execution on successful nightly run 38289151967
and **287 seconds** on 38149665434, plus runner preparation. The
remainder contained 388 existing prodex-app tests previously run serially
in one job. Increasing libtest threads within the runner would introduce
shared session, port and filesystem interference; independent serial
runner groups preserve that isolation.

The scheduled full-test matrix now has **four disjoint remainder
partitions**, each still executing with test-threads=1: continuation
and recovery (**66 tests**), remaining selection/admission and persistence
(**89**), core runtime foundation (**90**), and genuinely unassigned
tests (**143**). The broad selection partition explicitly skips all
existing targeted selection/admission prefixes; the final catch-all
skips all three new families in addition to the old targeted prefixes.
The ordinary push/PR prodex-app matrix, Windows matrix and base shard
manifest remain unchanged. Future new unassigned cases automatically
enter the catch-all rather than disappearing from CI.

Coverage was proven against a compiled prodex-app library test binary
listing **2,220 real cases**: each test had **exactly one** owner across
all 42 scheduled prodex-app partitions, including 388 remainder cases
with the above 66/89/90/143 breakdown. Permanent matrix regression
tests also check representative coverage, skip ownership, cache-writer
uniqueness, and an accurate compile-free dry run.

This change adds 3 full-test runner jobs in exchange for reducing the
single ~5-6 minute serial tail; it does not remove tests. Together with
the cached Mojo prerequisite, it should reduce the nightly suite
critical path, but actual end-to-end savings are not claimed until a new
scheduled or full manually dispatched run completes.


## Windows x64 ThinLTO codegen benchmark and CI fixture timing (October 14)

The successful official
[0.437.11 standalone release](https://github.com/christiandoxa/prodex/actions/runs/38504477460)
finished in about **14m 9s** with 19/19 jobs green. The dominant
single build was x86_64-pc-windows-msvc: **8m44s** compiling and **10m8s**
including setup/cache upload. Native Windows ARM64 compilation took 5m25s
and both macOS release builds finished in about 5 minutes each.
All six Mojo archives were verified or safely reused in less than 30 seconds.
The antivirus health check and release-asset scan took 39s and 58s;
neither security gate is optional.

The x64 Windows release compiler logged **281 sccache misses, zero hits**
(0% cache reuse) on the version-bumped source. Before changing critical
optimization flags or disabling a potentially useful cache, the
opt-in release-windows-x64-codegen-calibration.yml performs a controlled
A/B on the same official source SHA and its verified Mojo COFF archive.
Its two independent native Windows runners both build genuine ThinLTO:
16 codegen units (the default) and 64 codegen units, without sccache.
Both variants must validate the PE x64 architecture, version/help, and
compiled-in Mojo diagnostics; timing/size text is the only uploaded
artifact. No release or tag is modified. The official Windows runner, ThinLTO and package profile remain
unchanged while testing; more codegen units may reduce runtime optimizer
quality, so end-user performance is a distinct concern.

The [completed opt-in A/B run 38509522686](https://github.com/christiandoxa/prodex/actions/runs/38509522686)
confirmed both native Windows x64 binaries passed PE Machine 0x8664 and
version/help/Mojo runtime diagnostics, with no publication. Compiling
the official 0.437.11 source without sccache took **482 seconds at the
normal 16 codegen units**, compared with **510 seconds at 64 codegen
units**. The 16-unit binary was also smaller (36,400,640 bytes versus
36,803,584 bytes). The official 0.437.11 release compiler used
**524 seconds with sccache** and 0% hits. These are cross-run
comparisons, so the approximate **8% compilation speedup** from
removing the cold wrapper is observational, not a controlled same-run
wrapper A/B. Production therefore keeps the existing **ThinLTO and
16 codegen units** but skips sccache on Windows x64. The verified native
Windows ARM64 build already uses direct Cargo; macOS native builds
retain sccache, which sometimes has nonzero reuse.


On the CI side, [run 38503088881](https://github.com/christiandoxa/prodex/actions/runs/38503088881)
succeeded on the second attempt after two unchanged Windows fake-Kiro
ACP tests missed hard wall-clock assumptions on the first attempt.
The deterministic Python agent performs valid local protocol operations;
runner process startup and scheduling overhead can vary independently
from the Kiro ACP implementation. The fixture bootstrap therefore
allows up to 15 seconds on Windows but leaves the production 5-second
deadline unchanged. Two fixture response-time assertions allow 10 seconds
on Windows while retaining 2 seconds on Unix; the explicit fake-agent
linger is increased from 5 to 20 seconds on Windows so a regression
that incorrectly waits for child exit still fails. Timeout/termination
tests and child-invocation assertions remain intact; there are no
live provider requests. Preventing false-negative CI attempts avoids
a substantially more expensive rerun than a few seconds of test-only
scheduling slack.


### Remove unused host sccache from cross builds

Actual 0.437.11 release stats confirm that both Linux builds invoke the
cross container CLI directly and made **zero host sccache compiler requests**.
Installing the host compiler wrapper for those jobs wasted setup/download
and post-job bookkeeping, without any compilation cache benefit. Release
now sets up sccache only for the two native macOS release jobs;
Linux cross jobs continue to build identically with strict
compiled-in Mojo checks, archive ABI validation and native/QEMU smokes.
This is a small setup efficiency improvement, not a measured reduction in
the release's long Windows x64 critical path.


### Reuse third-party Rust object files across version-only release bumps

The x64 Windows release ran 281 cold sccache compiles while
Swatinem/rust-cache reported **No cache found**. The default
rust-cache key includes all workspace Cargo.toml and Cargo.lock
metadata, so changing every Prodex crate version invalidates the entire
compilation cache even when third-party dependencies are unchanged.

Directly comparing the official 0.437.10 and 0.437.11 Cargo.lock files
confirmed **434 identical external package identities/checksums**.
Only workspace version metadata changed. The new release cache helper
fingerprints **only** sorted external package names, versions, source URLs
and SHA-256 checksums. The fingerprint is unchanged across 0.437.10
and 0.437.11 (e4e1da8a0f43f36358bd0823a4e8f714f1b911c202fbe9c7b34b00662e6618c1)
but changes when a registry dependency version or checksum changes.

The release cache key explicitly pins Rust 1.99.0, ThinLTO, codegen
units, Mojo 1.1.0, target/CPU, OS and runner architecture, plus that
dependency fingerprint. Swatinem's automatic workspace-metadata hash
is disabled, **but workspace crates are explicitly excluded from the
cache** and Cargo still validates all compile fingerprints against
fresh source and lockfiles after restoring dependencies. A changed
compiler, target or dependency receives a different key; a workspace
code change is recompiled. All six build variants retain all their
existing release correctness, binary smoke and provenance gates.

This is a prospective acceleration; the first v1 cache must be saved
successfully before an identical dependency set can hit. The new
CI tests cover stable version-only identities, invalid source metadata,
checksum/version invalidation, and package-order independence.


### Windows CI compile-gate correction on the first candidate

The first checkpoint CI failed nine prodex-app Windows shards at Rust
compile time with E0425, before any Kiro fixture could run.
The test-only timeout constructor was already implemented on all
platforms, but its public(crate) test facade re-export was accidentally
gated behind cfg(all(test, unix)). The correction scopes that facade
to cfg(test) on all targets; production builds still cannot use it.
A mandatory source regression now rejects the Unix-only re-export.
The previous running CI result is not considered green; the corrected
commit requires new Windows CI verification.


## Critical-path refresh: 2026-10-14 14:00 WIB

[Push CI 38511339368](https://github.com/christiandoxa/prodex/actions/runs/38511339368)
finished with **74/74 successful jobs**. Strict Mojo archive caching has
removed the previous four-minute archive compiler barrier on unchanged
Mojo inputs. The slowest jobs are now independently serialized Windows
prodex-app suites:

| Windows suite | Measured total | Measured test step | Local test corpus |
| --- | ---: | ---: | ---: |
| Remaining selection/admission | 5.02 min | 4.15 min | 94 tests |
| Selection/providers/runtime | 4.42 min | 3.58 min | 711 tests |
| Gateway/support/affinity | 4.28 min | 2.78 min | 398 tests |

Rather than increasing libtest threads on stateful proxy/session tests,
the Windows-only generator partitions the targeted selection/admission,
provider launch and runtime-proxy groups into three independent serial
runners. The 94-case remainder is split into five explicit recovery
prefixes (52 cases) and a skip-filtered catch-all (42 cases). Future
new selection/admission modules remain in the catch-all. The generated
matrix now has **13 Windows partitions instead of 10**, with one Rust
cache writer, existing zero-test failure handling and identical platform
test flags.

Coverage was checked against the 2,240 cases in a runnable prodex-app
test binary: **2,240 assigned exactly once, zero missing and zero
duplicate**, including the original 711 and 94 affected cases. Permanent
source-level regression tests pin the 13 groups, all targeted filter
identities, 15 representative test owners and disjoint catch-all rules.
This is a wall-clock optimization at the cost of additional Windows
runner provisioning; per-shard and full-CI speedups are not claimed until
the first successful GitHub run with this matrix completes.

[Standalone release 38504477460](https://github.com/christiandoxa/prodex/actions/runs/38504477460)
finished in roughly 14 minutes with 19/19 successful jobs. Its six
strict archive jobs were already very short, but each still ran a
redundant sudo apt-get update and sudo apt-get install llvm even when
LLVM tools are provided by the Ubuntu image. The release archive setup
now probes for llvm-nm, llvm-ar and llvm-lib first, runs the original
install command only if any are missing, and then unconditionally
validates required tools and every archive symbol/security constraint.
Binary linking, six-platform smokes, checksum/SBOM/antivirus and
provenance gates are unchanged. This trims wasted package-manager work
and runner minutes, not the dominant Windows x64 ThinLTO link time.


## Benchmark Windows x64 release compilation on native ARM64 hosted runners

The official 0.437.11 standalone release spent **524 seconds** compiling
Windows x86_64 on the native x64 runner. The same source/arch/codegen16
benchmark without ineffective sccache took **482 seconds**; direct native
Windows ARM64 release compilation, by contrast, took 325-336 seconds
for the different ARM64 target. This suggests—but does not prove—that
the more capable ARM64 runner might accelerate x64 cross compilation.

An **opt-in, non-publishing** calibration workflow now checks out the
exact SHA of a completed-success official release, verifies its
repository/branch/workflow provenance, downloads the matching verified
x64 Mojo COFF archive, and attempts the same Rust 1.99.0 / ThinLTO /
16-codegen-unit x86_64-pc-windows-msvc release build on a windows-11-arm
runner. It verifies that the output is PE Machine 0x8664, then requires
the x64 binary to execute under Windows-on-ARM emulation with native
version/help and compiled-in Mojo runtime self-tests. Any unsupported
MSVC linker, x64 emulator, ABI, or runtime behavior fails the benchmark.

Only **text timing and binary-size evidence** is uploaded; the workflow
cannot publish assets, tags or packages. The official Windows x64 release
matrix remains unchanged until this calibration proves it safe and faster.
If the experiment fails, prefer the proven x64 host and maintain
ThinLTO/runtime diagnostics over any unverified speed gain.


## Measured Windows CI rebalancing after 13-shard checkpoint

[Push CI 38527770749](https://github.com/christiandoxa/prodex/actions/runs/38527770749)
finished **77/77 jobs successfully** with the previous 13 Windows
prodex-app partitions. The new slowest serial suites were:

| Windows CI suite | Total job | Serial test step | Cases in local binary |
| --- | ---: | ---: | ---: |
| Selection/admission | 4m31s | 3m14s | 231 |
| Local rewrite/profiles | 4m23s | 2m52s | 242 |
| Remainder | 4m14s | 3m26s | 143 |
| WebSocket continuation | 4m05s | 2m09s | 23 |

These times are influenced by runner and cache contention. Rather than
increasing libtest concurrency inside session-sensitive tests, the
manifest now divides selection/admission into independent 72-case
selection and 159-case admission-core runners, and local rewrite/profiles
into 131-case and 111-case runners. Each invocation remains **serial
within its own runner** with existing Windows exclusions and strict
zero-test detection. This increases the heavy Windows matrix from 13
to **15** runners, trading extra startup/billed minutes for a shorter
likely wall-clock critical path.

The actual compiled local prodex-app test binary lists **2,240 cases**.
The complete 15-shard manifest assigns **2,240/2,240 exactly once**,
with no missing or duplicate owners, and precisely one Rust cache
writer. A source-level regression pins the grouping and representative
runtime, admission, profile and local rewrite ownership. The
scheduled full-suite and Linux/macOS test matrices are unchanged.
The first completed CI run of the 15-shard configuration must be
measured before claiming a new wall-clock improvement.
