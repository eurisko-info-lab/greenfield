# Error model

## Canonical errors

The Machine uses explicit canonical errors for malformed binary input, including:

- `InvalidTag`
- `InvalidUtf8`
- `TrailingBytes`
- `NonCanonicalVarint`
- `DuplicateMapKey`
- `DepthLimit`
- `Unreachable`

## Delta errors

Structural change errors include:

- `TypeMismatch`
- `NotComposable`
- `InvalidIndex`
- `Unsupported(...)`

## Graph errors

Graph validation and closure checks may raise:

- `MissingDigest`
- `TypeMismatch`
- `InvalidRef`
- `InvalidType`
- `MissingType`
- `Conflict`

## Execution errors

Core evaluation may produce:

- `Returned(Value)`
- `Failed(String)`
- `Exhausted(String)`

The design emphasizes explicit failure classes over silent coercion. Invalid operations are rejected before state transition, and causal conflicts are reported directly rather than hidden behind host ordering assumptions.
