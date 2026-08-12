# Core IR and evaluation

## Core values and modules

The Machine uses a minimal graph-independent Core language for deterministic execution. The canonical module shape is:

- `CoreModule { functions: [CoreFunction] }`
- `CoreFunction { arity: Nat, body: Expr }`

The expression set is deliberately first-order and portable:

- `Literal(Value)`
- `Argument(Nat)`
- `Local(Nat)`
- `Let(Expr, Expr)`
- `Product([Expr])`
- `Sum(Nat, Expr)`
- `Match(Expr, [Expr])`
- `Primitive(PrimitiveId, [Expr])`
- `Call(FunctionIndex, [Expr])`
- `Effect(CapDigest, Expr)`

The machine performs call-by-value evaluation from left to right.

## Budget model

The budget is:

- `max_steps`
- `max_depth`
- `max_alloc` (optional)

The default F0 evaluation budget is finite and deterministic. Every expression dispatch, primitive invocation, function call and effect step consumes one unit of `max_steps` budget.

If a program exhausts the step budget, the result is a termination class, not a normal machine error.

## Outcome and verdict

The execution result is:

- `Returned(Value)`
- `Failed(CoreError)`
- `Exhausted(BudgetKind)`

The verdict records:

- `outcome`
- `steps`
- `receipts` in invocation order

The observation function records the semantic class and ordered receipt digests, without exposing performance counters.

## Primitives

The initial primitive set is intentionally small and fixed:

- boolean operations
- `u64` arithmetic with explicit overflow behavior
- bytes equality / concat / slice
- text equality
- product projection
- sequence length / index
- digest equality

No domain primitive belongs in F0.

## Effects

Effects are only invoked by capability digest. The initial bootstrap capabilities are deterministic and local, including `hash`, `cas_get`, and `cas_put`.

A receipt contains:

- capability digest
- handler digest
- request digest
- response digest
- status

The receipt order follows effect invocation order.
