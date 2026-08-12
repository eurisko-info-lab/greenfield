# Stratum architecture

## Governing law

The machine is intentionally boring. Meaning lives in graph data.

The system is specified by the following relations:

- Stratum = Graph + Machine
- L -> ΔL
- L × C_L -> Result(ΔL)
- history frontier != materialized state
- Observe(accelerated) = Observe(generic)
- Machine contains no application ontology

## Machine boundary

The Machine owns only canonical encoding, SHA-256, CAS, Core evaluation, budgets, bootstrap effect dispatch, accelerator dispatch, and optional local atomic heads.

Everything richer is graph-defined: types, typed nodes, arrows, languages, changes, claims, and future federation objects.

## Interoperability boundary

Rust and Scala are independent hosts. They do not share implementation source. They share documents, fixtures, canonical JSON outputs, and parity expectations.
