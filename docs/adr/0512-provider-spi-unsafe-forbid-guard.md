# ADR 0512: Guard provider SPI against unsafe code

## Status

Accepted

## Context

`prodex-provider-spi` is the transport-neutral contract between gateway routing and provider adapters. The crate already forbids unsafe code, but the CI boundary guard did not prove that invariant.

## Decision

The provider SPI boundary guard now requires `#![forbid(unsafe_code)]` in `crates/prodex-provider-spi/src/lib.rs`. Its self-test covers the accepted crate root and a negative fixture without the attribute.

## Consequences

Provider SPI contracts stay free of unsafe escape hatches unless the guard and ADR are deliberately changed.
