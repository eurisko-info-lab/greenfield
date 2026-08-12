# Laws and invariants

## Governing laws

The implementation follows the core Stratum laws:

- Stratum = Graph + Machine
- L -> ΔL
- L × C_L -> Result(ΔL)
- history frontier != materialized state
- Observe(accelerated) = Observe(generic)
- Machine contains no application ontology

## Canonicality laws

- canonical value encoding is unique and deterministic
- integer encoding is minimal and rejects non-minimal encodings
- map keys are ordered by canonical bytes
- duplicate keys are rejected
- trailing bytes are rejected
- depth is bounded

## Core laws

- evaluation is deterministic for the same Core program and input
- effect receipts reflect invocation order
- budget exhaustion is explicit and deterministic

## Delta laws

- `apply(a, zero(a)) = a`
- if `diff(a, b)` succeeds, then `apply(a, diff(a, b)) = b`
- if composition succeeds, then `apply(apply(a, d1), d2) = apply(a, compose(d1, d2))`

## History laws

- frontier tracks maximal causal nodes, not current state
- materialization remains stable for a fixed genesis and state pair
- non-commuting diamond operations produce an explicit conflict
