# Causal history

## Semantic model

A causal history is a graph-defined record of domain operations and their dependencies. It is used to reason about reachability, frontier materialization, and conflict detection without silently using host ordering or wall-clock time.

## Operation node

Each operation node is:

```text
OpNode {
  language_digest
  operation_digest
  dependencies: sorted set Digest
}
```

The full node digest identifies the operation event.

## History and frontier

A `History` is a vector of `OpNode`s. A `Frontier` is the set of maximal operation digests for the current causal cut.

Important rule:

- The frontier is not a materialized state digest.
- Materialization records genesis, frontier, and state digest separately.

## Materialization

The required materialization tuple is:

```text
Materialization {
  genesis_digest
  frontier
  state_digest
}
```

## Conflicts

If incomparable operations are not proven to commute, the system must return an explicit `Conflict` rather than silently applying a winner-takes-all rule.

The supporting law is:

- if `a || b` and `commute(a, b)`, then the two applications are equivalent regardless of ordering
- otherwise the system must record a conflict

This keeps history semantics faithful to causality and explicit operation dependencies.
