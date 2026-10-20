# Startup Probe Refresh Batch Selection Migrated to Mojo (2026-10-20)

## Production authority

The compiled runtime_state_quota.mojo owner now selects an entire ordered
batch of startup probe refresh candidates under one running warm-up budget
through the versioned prodex_runtime_state_startup_probe_batch_v1 ABI.
The kernel validates all observations before writing results and returns
a chosen flag for every candidate together with the remaining budget.

Rust acquires existing freshness and snapshot usability observations,
retains the original (possibly generic) profile names, divides arbitrarily
large collections into fixed 65,536-item ABI batches and applies the
Mojo-selected flags. The previous Rust per-candidate mutable budget
loop and individual policy call were removed; no Rust fallback for
selection order or budget exhaustion remains. The original public
single-candidate adapter remains an ABI compatibility surface, but the
live runtime-state refresh path now uses only the compiled batch planner.

## Verification

- Complete strict prodex-runtime-state suite: 25 tests pass, including
  one new caller regression with 65,540 candidates crossing the ABI
  batch boundary, exhausted budget, and no candidates.
- Two direct compiled-core tests verify exact stable order, no warm
  slots, empty input, 65,536 observations, excess-size rejection, bad
  version/count/capacity and late malformed flags before output writes.
- Direct Mojo ABI test exercises the same ordering and fail-closed
  behavior.
- Source anti-fallback guard/self-test, Cargo formatting/diff checks
  and official production-share non-regression validation pass.

The official production share changes from 35.83891367008976% to
35.837909642431626% (130,846 Mojo / 234,259 Rust production lines),
reflecting typed Rust ABI and caller acquisition code. The policy has
moved to Mojo; the project target 75% remains unmet. Counting rules
and historical floor were not weakened. Remote CI is not a checkpoint
prerequisite.
