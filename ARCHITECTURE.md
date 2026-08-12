# Stratum Architecture

## Core law

\[
\boxed{\text{Stratum}=\text{Graph}+\text{Machine}}
\]

The system is intentionally split between a boring Machine and a richer Graph layer. Meaning lives in graph data; the Machine only supplies irreducible mechanism.

## Machine boundary

The Machine owns:

- canonical encoding
- SHA-256 hashing
- CAS storage
- Core evaluation
- deterministic budgets
- bootstrap effect dispatch
- accelerator dispatch
- optional local atomic head

The Machine does not learn application ontology. It knows nothing about agents, studios, repositories, or domains beyond the frozen semantics in the canonical value algebra and Core evaluator.

## Graph boundary

Graph data carries all semantic structure, including:

- types and typed nodes
- references and closures
- arrows and lenses
- operation languages
- delta/change languages
- causal history and frontier data
- policies, claims, and future federation objects

The graph remains the place where domain intent is represented.

## Structural change law

\[
\boxed{L\mapsto\Delta L}
\]

Representation types have a canonical free structural delta language. This delta layer is intentionally simple and deterministic.

## Operation elaboration law

\[
\boxed{
L\times C_L\rightarrow\text{Result}(\Delta L)
}
\]

Operation languages are defined over state and operation types, and they elaborate lawful operations into structural deltas. Invalid operations are rejected before transition.

## History law

\[
\boxed{\text{history frontier}\neq\text{materialized state}}
\]

A frontier records maximal causal nodes; it is not a state digest. Materialization records genesis, frontier, and materialized state as separate observables.

## Accelerator law

\[
\boxed{
\text{Observe}(\text{accelerated}) = \text{Observe}(\text{generic})
}
\]

Accelerators are optional specialized implementations of pure arrows. If removed, the generic Core execution must preserve the same observable semantics.

## No application ontology rule

\[
\boxed{\text{Machine contains no application ontology}}
\]

If a concept would force the Machine to understand the application’s meaning, it belongs in the graph instead.

## Interoperability requirement

Rust and Scala are independent hosts. They share the spec, fixtures, canonical encoding rules, and parity expectations, but they do not share source code. Equivalent externally observable behavior is the required contract.
