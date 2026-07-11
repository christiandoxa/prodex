# ADR 0685: Gateway Credential Environment References Match Exactly

## Status

Accepted.

## Context

Configured gateway admin tokens and virtual keys reference credential material
through environment variable names. These references are secret-reference
inputs. Policy validation only rejected trim-empty values, and runtime config
resolution trimmed virtual-key `token_env` values before reading the
environment.

## Decision

Gateway credential environment references must now be exact non-empty values
without whitespace. Runtime config resolution no longer trims `token_env`
references; whitespace-bearing references fail closed or are ignored when
settings are built directly.

## Consequences

Canonical environment variable names are unchanged. Padded references no longer
resolve to credential material through trim-normalization.
