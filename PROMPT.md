# Stratum Greenfield Build Prompt

## Objective

Build **Stratum from scratch** as a minimal dual-host semantic system.

[
\boxed{\text{Stratum}=\text{Graph}+\text{Machine}}
]

Implement two independent hosts:

* **Rust**: reference implementation, optimized for clarity, explicit correctness and minimal dependencies.
* **Scala 3**: independent control implementation, optimized for semantic cross-checking against Rust.

Do **not** port, translate or reuse an existing Stratum, Granite, Cairn, Meta, repository or agent implementation.

The two hosts share **specifications, fixtures and required observations**, not implementation code.

---

# 1. Architectural law

The governing rule is:

[
\boxed{
\begin{gathered}
\text{Meaning lives in graph data.}\
\text{The Machine only provides irreducible mechanism.}
\end{gathered}
}
]

If a concept can be represented and interpreted as graph data, it must not become a feature-specific Machine API.

The Machine must never contain semantic feature names such as:

```text
Repository
Database
Agent
Investigation
SDS
Smalltalk
Foundation
Blockchain
Studio
```

Those may later be graph-defined applications.

---

# 2. Semantic split

## Machine

The Machine supplies only:

```text
canonical encoding
SHA-256
CAS
Core evaluation
deterministic budgets
bootstrap effect dispatch
accelerator dispatch
optional local atomic head
```

## Graph

Everything richer is graph data:

```text
types
typed nodes
typed references
arrows
lenses
languages
changes
domain operation languages
causal histories
branches/frontiers
policies
claims
accelerator manifests
future federation objects
```

---

# 3. Content graph versus semantic graph

Explicitly distinguish:

```text
ContentGraph
SemanticGraph
```

`ContentGraph` consists of immutable digest references stored in CAS and is structurally a content-addressed DAG.

Semantic cycles must be represented through graph-level identities, symbolic relationships, causal structures or local mutable heads.

Never assume arbitrary cyclic SHA-256 object graphs can be directly constructed.

---

# 4. Repository layout

Create:

```text
/
  host-rust/
  host-scala/

  fixtures/
    canon/
    core/
    graph/
    delta/
    history/

  docs/
    architecture.md
    canon.md
    core.md
    graph.md
    delta.md
    history.md
    laws.md
    errors.md

  tools/
    parity
```

Rust and Scala implementations must not import one another.

Fixtures and written specifications are the interoperability boundary.

---

# 5. F0: frozen Machine

Implement F0 completely before graph semantics.

## 5.1 Generic canonical Value

Use a self-describing generic value algebra:

```text
Value =
    Unit
  | Bool Bool
  | Nat u64
  | Bytes byte[]
  | Text UTF8
  | Sum tag Value
  | Product [Value]
  | Sequence [Value]
  | FiniteMap [(Value, Value)]
  | Digest byte[32]
  | Ref {
      digest      : Digest,
      type_digest : Digest
    }
```

No schema is required to decode a `Value`.

Schemas/types belong to F1.

---

# 6. Canonical binary encoding

Write the complete encoding in `docs/canon.md` before implementing it.

Requirements:

* every constructor has a fixed binary tag;
* integers use one exact canonical unsigned varint encoding;
* non-minimal integer encodings are rejected;
* `Text` is valid UTF-8 only;
* sequences encode length followed by elements;
* products preserve field order;
* sums encode tag followed by payload;
* maps are ordered by **canonical encoded key bytes**;
* duplicate canonical keys are rejected;
* `Digest` is exactly 32 bytes;
* `Ref` contains content digest followed by type digest;
* trailing bytes are rejected;
* decoding has an explicit nesting-depth limit.

Functions:

```text
encode(Value) -> Bytes
decode(Bytes) -> Value | CanonError
digest(Bytes) -> Digest
```

Digest:

```text
SHA-256(exact canonical bytes)
```

Both implementations must produce byte-for-byte identical encodings.

---

# 7. CAS

Implement:

```text
cas_put(Bytes) -> Digest
cas_get(Digest) -> Bytes | Missing
```

`cas_put` verifies nothing beyond hashing/storing bytes.

Typed validation belongs above CAS.

Use an in-memory implementation first.

Filesystem persistence is optional and must not affect semantics.

---

# 8. Core IR

Core is the only graph-independent executable language understood by the Machine.

Keep it deliberately small and first-order.

Use a canonical module:

```text
CoreModule {
  functions : Sequence CoreFunction
}

CoreFunction {
  arity : Nat
  body  : Expr
}
```

Suggested expression set:

```text
Expr =
    Literal Value
  | Argument Nat
  | Local Nat
  | Let Expr Expr
  | Product [Expr]
  | Sum Nat Expr
  | Match Expr [Expr]
  | Primitive PrimitiveId [Expr]
  | Call FunctionIndex [Expr]
  | Effect CapDigest Expr
```

Functions may recursively call themselves or one another.

No higher-order functions in v0.

No host-language closures in canonical semantics.

A graph arrow may later refer to:

```text
CoreArrow {
  module_digest
  function_index
}
```

---

# 9. Core evaluation semantics

Evaluation is:

* deterministic;
* call-by-value;
* left-to-right;
* independent of wall-clock time;
* completely documented in `docs/core.md`.

Define a deterministic abstract step model.

For example, charge fixed units for:

```text
expression dispatch
primitive invocation
function call
match dispatch
effect request
```

The precise accounting must be frozen in the specification.

Rust and Scala must report identical step counts for Core fixtures.

---

# 10. Budget and verdict

Define:

```text
Budget {
  max_steps
  max_depth
  optional max_alloc
}
```

Define:

```text
Outcome =
    Returned Value
  | Failed CoreError
  | Exhausted BudgetKind

Verdict {
  outcome
  steps
  receipts : Sequence Receipt
}
```

Budget exhaustion is not an ordinary Core error.

It is a termination class.

Define semantic observation:

```text
observe(Verdict) =
  outcome semantic class/value
  + ordered receipt digests
```

Performance counters are not part of `observe`.

---

# 11. Primitive operations

Keep primitives small and fixed.

Examples:

```text
bool operations
u64 arithmetic with explicit overflow behavior
bytes equality/concat/slice
text equality
product projection
sequence length/index
digest equality
```

Every primitive:

* has deterministic semantics;
* has explicit error behavior;
* has deterministic step cost.

No domain primitive belongs here.

---

# 12. Bootstrap effects

Effects are invoked only by capability digest.

Bootstrap capabilities may include:

```text
hash
cas_get
cas_put
log_trace
```

No network effect in v0.

Define:

```text
Receipt {
  capability_digest
  handler_digest
  request_digest
  response_digest
  status
}
```

The exact canonical Receipt schema must be shared.

Effect invocation order determines receipt order.

Fixture handlers must be deterministic.

---

# 13. F0 acceptance

F0 is complete only when:

* canonical bytes match across Rust and Scala;
* SHA-256 digests match;
* Core outcomes match;
* deterministic step counts match;
* receipt bytes and digests match;
* adversarial decode fixtures match;
* budget exhaustion classifications match.

Freeze F0 semantics after acceptance.

---

# 14. F1: typed CAS graph

Types are graph data, not new host classes with semantic behavior.

Define a minimal graph-level type description sufficient to validate generic `Value`s.

At minimum support:

```text
Unit
Bool
Nat
Bytes
Text
Digest
Sum
Product
Sequence
FiniteMap
Ref
Arrow
```

A typed node is:

```text
TypedNode {
  type_digest
  value
}
```

A typed reference remains:

```text
Ref {
  digest
  type_digest
}
```

Implement:

```text
check_type(Value, TypeDescription, Graph) -> Ok | TypeError
close(root_digest) -> Closure | GraphError
traverse(closure, path) -> Value | GraphError
```

`close` recursively follows graph-visible `Ref`s and verifies:

* referenced content exists;
* content digest matches;
* declared type digest exists;
* typed node validates.

Provide cross-host closure fixtures.

---

# 15. F2: arrows, lenses and minimal languages

A pure semantic arrow is a graph object referencing a Core entry:

```text
Arrow {
  input_type
  output_type
  core_module
  function_index
}
```

Implement arrow execution through `eval_core`.

Implement pure composition.

Define a lens as graph data:

```text
Lens {
  source_type
  view_type
  get_arrow
  modify_arrow
}
```

Use `modify` rather than assuming a mathematically total bidirectional `put`.

Lens laws are testable claims, not Machine axioms.

Examples:

```text
view after no-op modify
modify/view consistency where applicable
```

Define a minimal:

```text
Language {
  carrier_type
  named_lenses
}
```

Do not give `Language` privileged Machine behavior.

---

# 16. F3: free structural change

Derive a canonical structural change language:

[
L\mapsto\Delta L
]

for representation types only.

Support:

```text
Unit
Bool
Nat
Bytes
Product
Sum
Sequence
FiniteMap
```

The exact generated delta constructors must be specified in `docs/delta.md`.

Prefer simple, deterministic deltas over sophisticated minimal diffs.

A replacement operation is acceptable when necessary.

Required operations:

```text
zero(type)
apply(type, value, delta)
diff(type, before, after)
compose(type, delta1, delta2)
```

`compose` may return `NotComposable`.

Required laws:

[
apply(a,zero)=a
]

and when `diff` succeeds:

[
apply(a,diff(a,b))=b
]

and when composition succeeds:

[
apply(apply(a,d_1),d_2)
=======================

apply(a,compose(d_1,d_2))
]

Use deterministic property fixtures.

Rust and Scala must use the same explicit PRNG algorithm and seed when producing parity cases.

Do not rely on each language's default random generator.

---

# 17. F4: domain operation languages and reachability

Do not model domain operations only as a syntactic subset of `ΔL`.

Define:

```text
OperationLanguage C {
  operation_type
  state_type

  elaborate :
    State × Operation
      -> Result Delta

  optional static_restriction
}
```

Thus:

[
elaborate_L:L\times C_L\rightarrow Result(\Delta L)
]

and:

[
apply_C(s,c)
============

apply_\Delta(s,elaborate(s,c))
]

A literal restriction:

[
C\subseteq\Delta L
]

is a valid special case.

This allows state-dependent operations such as:

```text
Withdraw amount
Transfer source target amount
```

without granting arbitrary structural replacement.

Implement bounded reachability for testing:

```text
reachable(genesis, operations, depth)
```

Use it only as a testing/specification tool, not as a production enumeration mechanism.

Support invariant tests:

```text
invariant(genesis)

invariant(s) &&
apply_C(s,c) = s'
  => invariant(s')
```

---

# 18. F5: causal histories

Define operation nodes:

```text
OpNode {
  language_digest
  operation_digest
  dependencies : sorted set Digest
}
```

The node digest identifies the complete operation event.

Define:

```text
History
Frontier
Materialization
Conflict
```

A `Frontier` is the set of maximal operation-node digests.

It is not a state digest.

A materialization records:

```text
Materialization {
  genesis_digest
  frontier
  state_digest
}
```

---

# 19. Causal materialization law

Do not arbitrarily linearize concurrent operations.

For incomparable operations `a` and `b`, materialization may combine them only when the operation language declares or proves sufficient commutativity:

[
a\parallel b
\land
commute(a,b)
]

with:

[
apply_C(apply_C(s,a),b)
=======================

apply_C(apply_C(s,b),a)
]

Otherwise return an explicit:

```text
Conflict
```

Never silently resolve by:

```text
wall clock
digest ordering
host insertion order
last writer wins
```

unless a future graph-defined language explicitly chooses such a rule.

Required fixtures:

```text
linear history
diamond with commuting operations
diamond with conflicting operations
missing dependency
duplicate operation
invalid operation
```

Rust and Scala must derive identical frontier and materialization digests.

---

# 20. Accelerators: v0 interface only

Accelerators are optional specialized implementations of graph arrows.

Graph-level shape:

```text
AccelManifest {
  semantic_arrow
  source_closure_digest
  target_kind
  implementation_digest
}

AccelBinding {
  manifest_digest
  credential_digest optional
}
```

Host-local registry:

```text
accel_register(
  implementation_digest,
  implementation
)
```

Execution:

```text
accel_run(
  manifest,
  input,
  budget
) -> Verdict
```

For v0 support **pure arrows only**.

Required law:

[
\boxed{
Observe(accel(A,x))
===================

Observe(eval_core(A.semanticArrow,x))
}
]

Detaching every accelerator must preserve semantics.

No JIT is required.

A test accelerator may simply call an alternative hand-written implementation.

---

# 21. Deferred graph shapes

Do not implement these systems yet, but define their serializable graph types so future additions do not require changing F0.

## F6 candidate shapes

```text
CapabilityManifest
EffectPolicy
HistoricalReceipt
```

## F7

```text
Claim
EvidenceBundle
Constitution
Decision
```

## F8

```text
ExecutionProfile
Bottleneck
AccelerationContract
AccelerationNeed
```

## F9

```text
RunnerIdentity
ExecutionClaim
RunnerAttestation
AcceleratorCredential
CrookedNotice
```

## F10

```text
GraphChangeClaim
NodeAttestation
SettlementDecision
```

## F11

```text
RetentionConstitution
RetentionObligation
ReplayCapsule
ReconstructionRecipe
```

## F12

```text
Goal
CompiledContext
InvestigationTurn
InvestigationState
```

## F13

```text
StudioProjection
EditorProjection
```

## F14

```text
FoundationRoot
FoundationSuccessorClaim
```

These are graph data only.

No Machine service may be added for them.

---

# 22. Independent dual-host implementation

Rust is the reference host.

Scala is the independent control host.

They must share:

```text
docs
binary fixtures
expected digests
Core programs
graph fixtures
law fixtures
```

They must **not** share implementation source.

Do not translate Rust modules line-by-line into Scala.

Equivalent externally observable behavior is required, internal structure is deliberately free to differ.

---

# 23. Shared parity protocol

Each host must provide a tiny CLI:

```text
stratum-rust fixture <fixture-manifest>
stratum-scala fixture <fixture-manifest>
```

Each emits deterministic machine-readable records containing at least:

```text
fixture_id
encoded_hex
digest_hex
outcome_digest
steps
receipt_digests
optional graph_result_digest
```

`tools/parity` runs both and fails on any disagreement.

Do not use wall-clock data in parity output.

---

# 24. Testing

Use:

* Rust built-in tests plus a small property-testing dependency if useful;
* Scala MUnit or equivalent;
* shared fixed-seed generative cases;
* adversarial malformed binary fixtures;
* maximum nesting/budget cases;
* Core recursion exhaustion;
* effect receipt ordering;
* typed closure failures;
* delta law tests;
* causal diamond tests.

Every discovered cross-host discrepancy must become a permanent fixture before fixing it.

---

# 25. Documentation

Produce:

```text
docs/canon.md
docs/core.md
docs/graph.md
docs/delta.md
docs/history.md
docs/laws.md
docs/errors.md
ARCHITECTURE.md
```

`ARCHITECTURE.md` must fit on a few pages and state these laws prominently:

[
\boxed{\text{Stratum}=\text{Graph}+\text{Machine}}
]

[
\boxed{L\mapsto\Delta L}
]

[
\boxed{
L\times C_L
\rightarrow
Result(\Delta L)
}
]

[
\boxed{
\text{history frontier}
\neq
\text{materialized state}
}
]

[
\boxed{
Observe(accelerated)
====================

Observe(generic)
}
]

[
\boxed{
\text{Machine contains no application ontology}
}
]

---

# 26. Incremental build sequence

Do not attempt all foundations at once.

Implement in vertical slices:

```text
A. Canon encode/decode/digest
B. CAS
C. Core literals + primitives
D. Core calls + recursion + budgets
E. Core effects + receipts
F. F0 parity freeze

G. typed nodes + references
H. closure
I. F1 parity

J. arrows
K. lenses
L. F2 parity

M. free delta for scalar/product/sum
N. sequence/map delta
O. delta laws + parity

P. operation language C
Q. elaboration C -> Δ
R. reachability/invariants

S. linear causal history
T. commuting diamond
U. conflicting diamond
V. F5 acceptance
```

After every slice:

```text
cargo test
Scala tests
tools/parity
```

must all pass before proceeding.

---

# 27. Explicit non-goals

Do not implement:

```text
existing Stratum compatibility
Meta/Grammar compatibility
dependent type theory
network federation
blockchain consensus
agents
studios
LSP
UI
production JIT
production PKI
distributed runner market
database backend
self-hosting
```

Do not add speculative abstractions for them beyond the deferred graph shapes.

---

# 28. MVP acceptance

The build is accepted when all of the following hold:

1. Rust and Scala independently produce identical canonical bytes and SHA-256 digests for every shared fixture.
2. Core evaluation yields identical semantic outcomes, deterministic step counts and receipt digests.
3. F1 can close and validate a typed content graph.
4. Pure arrows and lenses execute through Core.
5. Free structural delta satisfies the specified laws.
6. A domain operation language (C) can elaborate lawful operations into structural deltas.
7. Invalid operations are rejected before state transition.
8. A linear causal history materializes correctly.
9. A commuting causal diamond converges independently of topological order.
10. A noncommuting diamond produces an explicit conflict.
11. A test accelerator is observationally equivalent to generic Core execution.
12. Removing all accelerators leaves every semantic test passing.
13. Host APIs contain no application/domain concepts.
14. The same fixture closure can be copied into an empty CAS and produces the same graph and evaluation results.

The MVP ends at F5 plus the minimal accelerator interface.

Do not continue into federation, retention, agents or self-hosting until this acceptance suite is green.

---

# 29. Governing design principle

When uncertain where functionality belongs, apply this test:

> **Would adding this feature require the frozen Machine to learn what the application means?**

If yes, stop and represent the concept in the graph instead.

The Machine should remain boring.

The graph is allowed to become interesting.
