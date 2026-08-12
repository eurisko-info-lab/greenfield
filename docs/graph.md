# Graph layer

## Purpose

The graph layer represents semantic structure that is not part of the frozen Machine. The Machine is intentionally minimal and only understands canonical bytes, SHA-256, CAS, Core evaluation, budgets, and effect receipts.

## Required graph types

The implementation distinguishes:

- `ContentGraph`: immutable CAS-backed digest references and content-addressed DAGs.
- `SemanticGraph`: graph-defined meaning such as types, closures, arrows, lenses, languages, and causal structures.

The graph layer defines the following families:

- `TypeDescription`: scalar and composite data types.
- `TypedNode`: value + type digest.
- `Graph`: a digest-indexed collection of typed nodes.
- `Arrow`: typed Core function with source and target types.
- `Lens`: a typed view/update pair built from `Arrow`s.
- `Language`: a graph-defined carrier type plus named lenses or operations.
- `OperationLanguage`: a domain-specific state transition language bound to a state type.
- `History`: a causal sequence of operation nodes.
- `Frontier`: the maximal set of operation-node digests for a history.
- `Materialization`: a digest triple recording genesis, frontier, and materialized state.
- `Conflict`: explicit non-commuting operation conflict.

## Laws

- Graph data carries semantic meaning.
- The Machine must not learn application ontology.
- Cycles must be represented with identities or local mutable heads rather than by assuming arbitrary cyclic SHA-256 graphs can be directly constructed.
- Structural closures may be computed from typed nodes and references, but semantic interpretation remains graph-defined.

## Observational intent

A graph is valid when its closure can be traversed, each typed node matches its expected type, and any produced materialization can be reproduced from the same genesis and frontier data without host-specific assumptions.
