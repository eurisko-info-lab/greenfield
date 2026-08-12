# Structural delta language

## Purpose

This document defines the canonical free structural change language `L -> ΔL` used for representation types only.

## Delta constructors

The deterministic delta set used by both hosts is:

- `Zero`
- `Replace(value)`
- `Product([delta])`
- `Sum { tag, delta }`
- `Sequence { index, value }`
- `MapInsert { key, value }`
- `MapRemove(key)`
- `BytesAppend(bytes)`

These are intentionally simple and deterministic. They prioritize clarity and parity over minimal edit scripts.

## Type coverage

Supported types include:

- `Unit`
- `Bool`
- `Nat`
- `Bytes`
- `Product`
- `Sum`
- `Sequence`
- `FiniteMap`

The delta model is deliberately not a general replacement for arbitrary graph semantics.

## Operations

Required operations:

- `zero(type)`
- `apply(type, value, delta)`
- `diff(type, before, after)`
- `compose(type, delta1, delta2)`

`compose` may return `NotComposable` for incompatible or non-associative cases.

## Laws

The implementation enforces:

- `apply(a, zero(a)) = a`
- if `diff(a, b)` succeeds, then `apply(a, diff(a, b)) = b`
- if composition succeeds, then `apply(apply(a, d1), d2) = apply(a, compose(d1, d2))`

The delta model is intentionally structural; it does not encode arbitrary domain semantics.
