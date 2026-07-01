# ADR 0513: Guard control plane against unsafe code

## Status

Accepted

## Context

`prodex-control-plane` owns admin, tenant, identity, key, billing, audit, and configuration-management planning. It already declares `#![forbid(unsafe_code)]`, but the boundary guard did not enforce that invariant.

## Decision

The control-plane boundary guard now requires `#![forbid(unsafe_code)]` in `crates/prodex-control-plane/src/lib.rs`. The self-test covers both the valid crate root and a negative fixture without the attribute.

## Consequences

Control-plane contract code stays free of unsafe escape hatches unless the ADR and guard are deliberately changed.
