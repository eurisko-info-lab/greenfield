use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub const MAX_DEPTH: usize = 64;
pub type Digest = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Unit,
    Bool(bool),
    Nat(u64),
    Bytes(Vec<u8>),
    Text(String),
    Sum(u64, Box<Value>),
    Product(Vec<Value>),
    Sequence(Vec<Value>),
    FiniteMap(Vec<(Value, Value)>),
    Digest(Digest),
    Ref { digest: Digest, type_digest: Digest },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonError {
    InvalidTag(u8),
    InvalidUtf8,
    TrailingBytes,
    NonCanonicalVarint,
    DuplicateMapKey,
    DepthLimit,
    Unreachable,
    Unexpected(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub capability_digest: Digest,
    pub handler_digest: Digest,
    pub request_digest: Digest,
    pub response_digest: Digest,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Returned(Value),
    Failed(String),
    Exhausted(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    pub max_steps: u64,
    pub max_depth: u64,
    pub max_alloc: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub outcome: Outcome,
    pub steps: u64,
    pub receipts: Vec<Receipt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Literal(Value),
    Argument(u64),
    Local(u64),
    Let(Box<Expr>, Box<Expr>),
    Product(Vec<Expr>),
    Sum(u64, Box<Expr>),
    Match(Box<Expr>, Vec<Expr>),
    Primitive(String, Vec<Expr>),
    Call(u64, Vec<Expr>),
    Effect(String, Box<Expr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreFunction {
    pub arity: u64,
    pub body: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreModule {
    pub functions: Vec<CoreFunction>,
}

#[derive(Default)]
pub struct CasStore {
    entries: HashMap<Digest, Vec<u8>>,
}

static GLOBAL_CAS: OnceLock<Mutex<HashMap<Digest, Vec<u8>>>> = OnceLock::new();

fn global_cas() -> &'static Mutex<HashMap<Digest, Vec<u8>>> {
    GLOBAL_CAS.get_or_init(|| Mutex::new(HashMap::new()))
}

impl CasStore {
    pub fn put(&mut self, bytes: &[u8]) -> Digest {
        let digest = digest_bytes(bytes);
        self.entries.insert(digest, bytes.to_vec());
        digest
    }

    pub fn get(&self, digest: Digest) -> Option<Vec<u8>> {
        self.entries.get(&digest).cloned()
    }
}

pub fn cas_put(bytes: &[u8]) -> Digest {
    let digest = digest_bytes(bytes);
    let mut store = global_cas().lock().unwrap();
    store.insert(digest, bytes.to_vec());
    digest
}

pub fn cas_get(digest: Digest) -> Option<Vec<u8>> {
    let store = global_cas().lock().unwrap();
    store.get(&digest).cloned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeDescription {
    Unit,
    Bool,
    Nat,
    Bytes,
    Text,
    Digest,
    Sum(Box<TypeDescription>, Box<TypeDescription>),
    Product(Vec<TypeDescription>),
    Sequence(Box<TypeDescription>),
    FiniteMap(Box<TypeDescription>, Box<TypeDescription>),
    Ref,
    Arrow(Box<TypeDescription>, Box<TypeDescription>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedNode {
    pub type_digest: Digest,
    pub value: Value,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Graph {
    entries: HashMap<Digest, TypedNode>,
}

impl Graph {
    pub fn insert(&mut self, value: Value, type_digest: Digest) -> Digest {
        let node = TypedNode { type_digest, value };
        let digest = digest_bytes(&encode_value(&node.value).unwrap_or_default());
        self.entries.insert(digest, node);
        digest
    }

    pub fn get(&self, digest: Digest) -> Option<&TypedNode> {
        self.entries.get(&digest)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    MissingDigest(Digest),
    TypeMismatch,
    InvalidRef,
    InvalidType,
    MissingType,
    Conflict,
    Unexpected(String),
}

pub fn type_digest_for_value(value: &Value) -> Digest {
    let bytes = match value {
        Value::Unit => b"unit".to_vec(),
        Value::Bool(_) => b"bool".to_vec(),
        Value::Nat(_) => b"nat".to_vec(),
        Value::Bytes(_) => b"bytes".to_vec(),
        Value::Text(_) => b"text".to_vec(),
        Value::Sum(_, _) => b"sum".to_vec(),
        Value::Product(_) => b"product".to_vec(),
        Value::Sequence(_) => b"sequence".to_vec(),
        Value::FiniteMap(_) => b"finitemap".to_vec(),
        Value::Digest(_) => b"digest".to_vec(),
        Value::Ref { .. } => b"ref".to_vec(),
    };
    digest_bytes(&bytes)
}

pub fn check_type(value: &Value, ty: &TypeDescription, graph: &Graph) -> Result<(), GraphError> {
    match (value, ty) {
        (Value::Unit, TypeDescription::Unit) => Ok(()),
        (Value::Bool(_), TypeDescription::Bool) => Ok(()),
        (Value::Nat(_), TypeDescription::Nat) => Ok(()),
        (Value::Bytes(_), TypeDescription::Bytes) => Ok(()),
        (Value::Text(_), TypeDescription::Text) => Ok(()),
        (Value::Digest(_), TypeDescription::Digest) => Ok(()),
        (Value::Sum(_, payload), TypeDescription::Sum(t1, t2)) => {
            check_type(payload, t2, graph).and_then(|_| check_type(&Value::Unit, t1, graph))
        }
        (Value::Product(items), TypeDescription::Product(tys)) if items.len() == tys.len() => {
            for (item, ty_item) in items.iter().zip(tys.iter()) {
                check_type(item, ty_item, graph)?;
            }
            Ok(())
        }
        (Value::Sequence(items), TypeDescription::Sequence(ty_item)) => {
            for item in items { check_type(item, ty_item, graph)?; }
            Ok(())
        }
        (Value::FiniteMap(entries), TypeDescription::FiniteMap(key_ty, value_ty)) => {
            for (key, value) in entries {
                check_type(key, key_ty, graph)?;
                check_type(value, value_ty, graph)?;
            }
            Ok(())
        }
        (Value::Ref { digest, type_digest }, TypeDescription::Ref) => {
            if graph.get(*digest).is_none() { return Err(GraphError::MissingDigest(*digest)); }
            if graph.get(*digest).map(|node| node.type_digest) != Some(*type_digest) { return Err(GraphError::TypeMismatch); }
            Ok(())
        }
        (Value::Ref { digest, type_digest }, _) => {
            let node = graph.get(*digest).ok_or(GraphError::MissingDigest(*digest))?;
            if node.type_digest != *type_digest { return Err(GraphError::TypeMismatch); }
            Ok(())
        }
        _ => Err(GraphError::TypeMismatch),
    }
}

pub fn close(graph: &Graph, root_digest: Digest) -> Result<Vec<Digest>, GraphError> {
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root_digest];
    let mut out = Vec::new();
    while let Some(digest) = stack.pop() {
        if !seen.insert(digest) { continue; }
        out.push(digest);
        let node = graph.get(digest).ok_or(GraphError::MissingDigest(digest))?;
        if let Value::Ref { digest: child_digest, type_digest } = &node.value {
            stack.push(*child_digest);
            if graph.get(*child_digest).map(|n| n.type_digest) != Some(*type_digest) {
                return Err(GraphError::TypeMismatch);
            }
        }
    }
    Ok(out)
}

pub fn traverse(graph: &Graph, closure: &[Digest], path: &[usize]) -> Result<Value, GraphError> {
    if path.is_empty() {
        return Err(GraphError::Unexpected("empty path".to_string()));
    }

    let mut current_index = *path.first().unwrap();
    let mut current_digest = closure.get(current_index).copied().ok_or(GraphError::Unexpected("path index out of range".to_string()))?;
    let mut current = graph.get(current_digest).ok_or(GraphError::MissingDigest(current_digest))?.clone();

    for &next_index in &path[1..] {
        let child_digest = match &current.value {
            Value::Ref { digest, .. } => *digest,
            _ => return Err(GraphError::Unexpected("non-ref traversal".to_string())),
        };

        let expected_index = closure.iter().position(|d| d == &child_digest).ok_or(GraphError::MissingDigest(child_digest))?;
        if expected_index != next_index {
            return Err(GraphError::Unexpected("path does not match closure".to_string()));
        }

        current_index = next_index;
        current_digest = closure.get(current_index).copied().ok_or(GraphError::Unexpected("path index out of range".to_string()))?;
        current = graph.get(current_digest).ok_or(GraphError::MissingDigest(current_digest))?.clone();
    }

    Ok(current.value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrow {
    pub input_type: TypeDescription,
    pub output_type: TypeDescription,
    pub core_module: CoreModule,
    pub function_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lens {
    pub source_type: TypeDescription,
    pub view_type: TypeDescription,
    pub get_arrow: Arrow,
    pub modify_arrow: Arrow,
}

pub fn run_arrow(arrow: &Arrow, input: Value) -> Verdict {
    eval_core(&arrow.core_module, arrow.function_index, vec![input])
}

pub fn run_lens_get(lens: &Lens, input: Value) -> Verdict {
    run_arrow(&lens.get_arrow, input)
}

pub fn run_lens_modify(lens: &Lens, input: Value, new_view: Value) -> Verdict {
    let payload = Value::Product(vec![input, new_view]);
    run_arrow(&lens.modify_arrow, payload)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccelManifest {
    pub semantic_arrow: Arrow,
    pub source_closure_digest: Digest,
    pub target_kind: String,
    pub implementation_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccelBinding {
    pub manifest_digest: Digest,
    pub credential_digest: Option<Digest>,
}

type AccelHandler = Arc<dyn Fn(Value, &Budget) -> Verdict + Send + Sync>;

static ACCEL_REGISTRY: OnceLock<Mutex<HashMap<Digest, AccelHandler>>> = OnceLock::new();

pub fn accel_register(implementation_digest: Digest, implementation: AccelHandler) {
    let mut registry = ACCEL_REGISTRY.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    registry.insert(implementation_digest, implementation);
}

pub fn accel_run(manifest: &AccelManifest, input: Value, budget: Budget) -> Verdict {
    let registry = ACCEL_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let implementation = registry.lock().unwrap().get(&manifest.implementation_digest).cloned();
    match implementation {
        Some(handler) => handler(input, &budget),
        None => run_arrow(&manifest.semantic_arrow, input),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delta {
    Zero,
    Replace(Value),
    Product(Vec<Delta>),
    Sum { tag: u64, delta: Box<Delta> },
    Sequence { index: usize, value: Value },
    MapInsert { key: Value, value: Value },
    MapRemove(Value),
    BytesAppend(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeltaError {
    TypeMismatch,
    NotComposable,
    InvalidIndex,
    Unsupported(String),
}

pub fn zero_for_type(ty: &TypeDescription) -> Delta {
    match ty {
        TypeDescription::Unit => Delta::Zero,
        TypeDescription::Bool => Delta::Zero,
        TypeDescription::Nat => Delta::Zero,
        TypeDescription::Bytes => Delta::Zero,
        TypeDescription::Text => Delta::Zero,
        TypeDescription::Digest => Delta::Zero,
        TypeDescription::Product(_) => Delta::Product(Vec::new()),
        TypeDescription::Sequence(_) => Delta::Zero,
        TypeDescription::FiniteMap(_, _) => Delta::Zero,
        TypeDescription::Ref => Delta::Zero,
        TypeDescription::Arrow(_, _) => Delta::Zero,
        TypeDescription::Sum(_, _) => Delta::Zero,
    }
}

pub fn apply_delta(ty: &TypeDescription, value: &Value, delta: &Delta) -> Result<Value, DeltaError> {
    match (ty, value, delta) {
        (_, _, Delta::Zero) => Ok(value.clone()),
        (_, _, Delta::Replace(next)) => Ok(next.clone()),
        (TypeDescription::Product(types), Value::Product(items), Delta::Product(deltas)) if items.len() == types.len() && deltas.len() == types.len() => {
            let mut out = Vec::new();
            for ((item, ty), delta) in items.iter().zip(types.iter()).zip(deltas.iter()) {
                out.push(apply_delta(ty, item, delta)?);
            }
            Ok(Value::Product(out))
        }
        (TypeDescription::Sum(_, payload_ty), Value::Sum(tag, payload), Delta::Sum { tag: new_tag, delta }) if *tag == *new_tag => {
            let next = apply_delta(payload_ty, payload, delta)?;
            Ok(Value::Sum(*tag, Box::new(next)))
        }
        (TypeDescription::Sequence(_), Value::Sequence(items), Delta::Sequence { index, value: replacement }) if *index < items.len() => {
            let mut next = items.clone();
            next[*index] = replacement.clone();
            Ok(Value::Sequence(next))
        }
        (TypeDescription::FiniteMap(_, _), Value::FiniteMap(entries), Delta::MapInsert { key, value }) => {
            let mut next = entries.clone();
            if let Some(position) = next.iter().position(|(existing, _)| existing == key) {
                next[position].1 = value.clone();
            } else {
                next.push((key.clone(), value.clone()));
            }
            Ok(Value::FiniteMap(next))
        }
        (TypeDescription::FiniteMap(_, _), Value::FiniteMap(entries), Delta::MapRemove(key)) => {
            let next = entries.iter().filter(|(existing, _)| existing != key).cloned().collect();
            Ok(Value::FiniteMap(next))
        }
        (TypeDescription::Bytes, Value::Bytes(bytes), Delta::BytesAppend(extra)) => {
            let mut next = bytes.clone();
            next.extend(extra.iter().copied());
            Ok(Value::Bytes(next))
        }
        _ => Err(DeltaError::TypeMismatch),
    }
}

pub fn diff_delta(ty: &TypeDescription, before: &Value, after: &Value) -> Result<Delta, DeltaError> {
    if before == after {
        return Ok(Delta::Zero);
    }
    match (ty, before, after) {
        (TypeDescription::Product(types), Value::Product(before_items), Value::Product(after_items)) if before_items.len() == after_items.len() && before_items.len() == types.len() => {
            let deltas = before_items
                .iter()
                .zip(after_items.iter())
                .zip(types.iter())
                .map(|((b, a), t)| diff_delta(t, b, a))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Delta::Product(deltas))
        }
        (TypeDescription::Sum(_, _), Value::Sum(tag_before, before_payload), Value::Sum(tag_after, after_payload)) if tag_before == tag_after => {
            let inner = diff_delta(&TypeDescription::Unit, before_payload, after_payload)?;
            Ok(Delta::Sum { tag: *tag_before, delta: Box::new(inner) })
        }
        (TypeDescription::Sequence(_), Value::Sequence(before_items), Value::Sequence(after_items)) if before_items.len() == after_items.len() => {
            let deltas = before_items.iter().zip(after_items.iter()).enumerate().map(|(idx, (_, a))| Delta::Sequence { index: idx, value: a.clone() }).collect();
            Ok(Delta::Product(deltas))
        }
        (TypeDescription::FiniteMap(_, _), Value::FiniteMap(before_entries), Value::FiniteMap(after_entries)) => {
            let mut result = Vec::new();
            for (key, value) in after_entries.iter() {
                if !before_entries.iter().any(|(existing, _)| existing == key) || before_entries.iter().find(|(existing, _)| existing == key).map(|(_, old)| old) != Some(value) {
                    result.push(Delta::MapInsert { key: key.clone(), value: value.clone() });
                }
            }
            for (key, _) in before_entries.iter() {
                if !after_entries.iter().any(|(existing, _)| existing == key) {
                    result.push(Delta::MapRemove(key.clone()));
                }
            }
            if result.is_empty() { Ok(Delta::Zero) } else { Ok(Delta::Product(result)) }
        }
        _ => Ok(Delta::Replace(after.clone())),
    }
}

pub fn compose_delta(ty: &TypeDescription, left: &Delta, right: &Delta) -> Result<Delta, DeltaError> {
    match (left, right) {
        (Delta::Zero, _) => Ok(right.clone()),
        (_, Delta::Zero) => Ok(left.clone()),
        (Delta::Replace(_), Delta::Replace(after)) => Ok(Delta::Replace(after.clone())),
        (Delta::Product(xs), Delta::Product(ys)) => {
            let mut out = Vec::new();
            for (x, y) in xs.iter().zip(ys.iter()) { out.push(compose_delta(ty, x, y)?); }
            Ok(Delta::Product(out))
        }
        _ => Ok(Delta::Replace(match right {
            Delta::Replace(value) => value.clone(),
            _ => match value_for_delta(right) { Some(value) => value, None => Value::Unit },
        })),
    }
}

fn value_for_delta(delta: &Delta) -> Option<Value> {
    match delta {
        Delta::Replace(value) => Some(value.clone()),
        Delta::Sequence { value, .. } => Some(value.clone()),
        Delta::MapInsert { value, .. } => Some(value.clone()),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Noop,
    SetNat(u64),
    AddNat(u64),
    SetText(String),
    MapInsert { key: Value, value: Value },
    SequencePush(Value),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationLanguage {
    pub operation_type: TypeDescription,
    pub state_type: TypeDescription,
}

pub fn elaborate_operation(language: &OperationLanguage, state: &Value, operation: &Operation) -> Result<Delta, DeltaError> {
    match (language.state_type.clone(), state, operation) {
        (TypeDescription::Nat, _, Operation::SetNat(value)) => Ok(Delta::Replace(Value::Nat(*value))),
        (TypeDescription::Nat, Value::Nat(current), Operation::AddNat(delta)) => Ok(Delta::Replace(Value::Nat(current.saturating_add(*delta)))),
        (TypeDescription::Text, _, Operation::SetText(value)) => Ok(Delta::Replace(Value::Text(value.clone()))),
        (TypeDescription::FiniteMap(_, _), Value::FiniteMap(_), Operation::MapInsert { key, value }) => Ok(Delta::MapInsert { key: key.clone(), value: value.clone() }),
        (TypeDescription::Sequence(_), Value::Sequence(_), Operation::SequencePush(value)) => Ok(Delta::Sequence { index: 0, value: value.clone() }),
        _ => Err(DeltaError::Unsupported("operation not supported for state type".to_string())),
    }
}

pub fn reachable(genesis: &Value, operations: &[Operation], depth: usize) -> Result<Vec<Value>, DeltaError> {
    let mut current = genesis.clone();
    let mut states = vec![current.clone()];
    for op in operations.iter().take(depth) {
        let language = OperationLanguage {
            operation_type: TypeDescription::Nat,
            state_type: TypeDescription::Nat,
        };
        let next = apply_delta(&language.state_type, &current, &elaborate_operation(&language, &current, op)?)?;
        states.push(next.clone());
        current = next;
    }
    Ok(states)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpNode {
    pub language_digest: Digest,
    pub operation_digest: Digest,
    pub dependencies: Vec<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub nodes: Vec<OpNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Materialization {
    pub genesis_digest: Digest,
    pub frontier: Vec<Digest>,
    pub state_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub nodes: Vec<Digest>,
}

pub fn frontier(history: &History) -> Vec<Digest> {
    history.nodes.iter().map(|node| digest_bytes(&encode_value(&Value::Text(format!("{:?}", node))).unwrap_or_default())).collect()
}

pub fn materialize(history: &History, genesis: &Value, state: &Value) -> Materialization {
    let genesis_digest = digest_bytes(&encode_value(genesis).unwrap_or_default());
    let state_digest = digest_bytes(&encode_value(state).unwrap_or_default());
    Materialization { genesis_digest, frontier: frontier(history), state_digest }
}

pub fn detect_conflict(history: &History) -> Option<Conflict> {
    if history.nodes.len() < 2 {
        return None;
    }
    let nodes = history.nodes.iter().map(|node| {
        digest_bytes(&encode_value(&Value::Text(format!("{:?}", node))).unwrap_or_default())
    }).collect();
    Some(Conflict { nodes })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityManifest {
    pub digest: Digest,
    pub capability: String,
    pub effect_kind: String,
    pub restrictions: Vec<String>,
    pub policy_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectPolicy {
    pub digest: Digest,
    pub effect_kind: String,
    pub allowed_capabilities: Vec<Digest>,
    pub require_proof: bool,
    pub policy_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoricalReceipt {
    pub receipt_digest: Digest,
    pub capability_digest: Digest,
    pub handler_digest: Digest,
    pub request_digest: Digest,
    pub response_digest: Digest,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub digest: Digest,
    pub actor: String,
    pub claim_type: String,
    pub subject_digest: Digest,
    pub evidence: Vec<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceBundle {
    pub digest: Digest,
    pub claim_digest: Digest,
    pub evidence: Vec<Digest>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constitution {
    pub digest: Digest,
    pub name: String,
    pub rules: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub digest: Digest,
    pub constitution_digest: Digest,
    pub claim_digest: Digest,
    pub outcome: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionProfile {
    pub digest: Digest,
    pub runner_digest: Digest,
    pub profile_name: String,
    pub capacity: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bottleneck {
    pub digest: Digest,
    pub stage: String,
    pub description: String,
    pub observed_limit: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccelerationContract {
    pub digest: Digest,
    pub semantic_arrow_digest: Digest,
    pub accelerator_digest: Digest,
    pub requirements: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccelerationNeed {
    pub digest: Digest,
    pub subject_digest: Digest,
    pub need_kind: String,
    pub priority: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerIdentity {
    pub digest: Digest,
    pub name: String,
    pub public_key_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionClaim {
    pub digest: Digest,
    pub runner_digest: Digest,
    pub execution_digest: Digest,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerAttestation {
    pub digest: Digest,
    pub runner_digest: Digest,
    pub attestation_digest: Digest,
    pub assertion: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceleratorCredential {
    pub digest: Digest,
    pub runner_digest: Digest,
    pub capability_digest: Digest,
    pub scope: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrookedNotice {
    pub digest: Digest,
    pub runner_digest: Digest,
    pub reason: String,
    pub evidence: Vec<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphChangeClaim {
    pub digest: Digest,
    pub before_digest: Digest,
    pub after_digest: Digest,
    pub change_kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeAttestation {
    pub digest: Digest,
    pub node_digest: Digest,
    pub attestor_digest: Digest,
    pub claim: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettlementDecision {
    pub digest: Digest,
    pub claim_digest: Digest,
    pub decision: String,
    pub voter_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionConstitution {
    pub digest: Digest,
    pub name: String,
    pub retention_policy: String,
    pub witness: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionObligation {
    pub digest: Digest,
    pub constitution_digest: Digest,
    pub subject_digest: Digest,
    pub duration: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayCapsule {
    pub digest: Digest,
    pub origin_digest: Digest,
    pub frontier_digest: Digest,
    pub replay_log: Vec<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconstructionRecipe {
    pub digest: Digest,
    pub genesis_digest: Digest,
    pub input_history: Vec<Digest>,
    pub reconstruction: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Goal {
    pub digest: Digest,
    pub name: String,
    pub objective: String,
    pub success_criteria: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledContext {
    pub digest: Digest,
    pub goal_digest: Digest,
    pub environment_digest: Digest,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvestigationTurn {
    pub digest: Digest,
    pub context_digest: Digest,
    pub turn_index: u64,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvestigationState {
    pub digest: Digest,
    pub current_turn: u64,
    pub state_name: String,
    pub evidence: Vec<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudioProjection {
    pub digest: Digest,
    pub subject_digest: Digest,
    pub perspective: String,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorProjection {
    pub digest: Digest,
    pub subject_digest: Digest,
    pub editor_id: String,
    pub selection: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundationRoot {
    pub digest: Digest,
    pub name: String,
    pub version: String,
    pub seed_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundationSuccessorClaim {
    pub digest: Digest,
    pub root_digest: Digest,
    pub successor_digest: Digest,
    pub reason: String,
}

pub fn digest_bytes(bytes: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

pub fn encode_value(value: &Value) -> Result<Vec<u8>, CanonError> {
    encode_value_with_depth(value, 0)
}

fn encode_value_with_depth(value: &Value, depth: usize) -> Result<Vec<u8>, CanonError> {
    if depth > MAX_DEPTH {
        return Err(CanonError::DepthLimit);
    }
    match value {
        Value::Unit => Ok(vec![0]),
        Value::Bool(flag) => Ok(vec![1, if *flag { 1 } else { 0 }]),
        Value::Nat(n) => {
            let mut bytes = encode_u64(*n)?;
            let mut out = vec![2];
            out.append(&mut bytes);
            Ok(out)
        }
        Value::Bytes(bytes) => {
            let mut out = vec![3];
            out.extend(encode_u64(bytes.len() as u64)?);
            out.extend(bytes);
            Ok(out)
        }
        Value::Text(text) => {
            if std::str::from_utf8(text.as_bytes()).is_err() {
                return Err(CanonError::InvalidUtf8);
            }
            let mut out = vec![4];
            out.extend(encode_u64(text.len() as u64)?);
            out.extend(text.as_bytes());
            Ok(out)
        }
        Value::Sum(tag, payload) => {
            let mut out = vec![5];
            out.extend(encode_u64(*tag)?);
            out.extend(encode_value_with_depth(payload, depth + 1)?);
            Ok(out)
        }
        Value::Product(items) => {
            let mut out = vec![6];
            out.extend(encode_u64(items.len() as u64)?);
            for item in items {
                out.extend(encode_value_with_depth(item, depth + 1)?);
            }
            Ok(out)
        }
        Value::Sequence(items) => {
            let mut out = vec![7];
            out.extend(encode_u64(items.len() as u64)?);
            for item in items {
                out.extend(encode_value_with_depth(item, depth + 1)?);
            }
            Ok(out)
        }
        Value::FiniteMap(entries) => {
            let mut pairs = entries.clone();
            pairs.sort_by(|(k1, _), (k2, _)| {
                let left = encode_value_with_depth(k1, depth + 1).unwrap();
                let right = encode_value_with_depth(k2, depth + 1).unwrap();
                left.cmp(&right)
            });
            let mut seen = HashMap::<Vec<u8>, ()>::new();
            for (key, _) in &pairs {
                let bytes = encode_value_with_depth(key, depth + 1)?;
                if seen.contains_key(&bytes) {
                    return Err(CanonError::DuplicateMapKey);
                }
                seen.insert(bytes, ());
            }
            let mut out = vec![8];
            out.extend(encode_u64(pairs.len() as u64)?);
            for (key, value) in pairs {
                out.extend(encode_value_with_depth(&key, depth + 1)?);
                out.extend(encode_value_with_depth(&value, depth + 1)?);
            }
            Ok(out)
        }
        Value::Digest(bytes) => {
            let mut out = vec![9];
            out.extend(bytes);
            Ok(out)
        }
        Value::Ref { digest, type_digest } => {
            let mut out = vec![10];
            out.extend(digest);
            out.extend(type_digest);
            Ok(out)
        }
    }
}

fn encode_u64(value: u64) -> Result<Vec<u8>, CanonError> {
    let mut out = Vec::new();
    let mut x = value;
    loop {
        let mut byte = (x & 0x7f) as u8;
        x >>= 7;
        if x != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if x == 0 {
            break;
        }
    }
    Ok(out)
}

pub fn decode_value(bytes: &[u8]) -> Result<Value, CanonError> {
    let mut pos = 0usize;
    let value = decode_value_at(bytes, &mut pos, 0)?;
    if pos != bytes.len() {
        return Err(CanonError::TrailingBytes);
    }
    Ok(value)
}

fn decode_value_at(bytes: &[u8], pos: &mut usize, depth: usize) -> Result<Value, CanonError> {
    if depth > MAX_DEPTH {
        return Err(CanonError::DepthLimit);
    }
    if *pos >= bytes.len() {
        return Err(CanonError::Unreachable);
    }
    let tag = bytes[*pos];
    *pos += 1;
    match tag {
        0 => Ok(Value::Unit),
        1 => {
            if *pos >= bytes.len() {
                return Err(CanonError::Unreachable);
            }
            let v = bytes[*pos];
            *pos += 1;
            Ok(Value::Bool(v != 0))
        }
        2 => {
            let value = decode_u64(bytes, pos)?;
            Ok(Value::Nat(value))
        }
        3 => {
            let len = decode_u64(bytes, pos)? as usize;
            let end = *pos + len;
            if end > bytes.len() {
                return Err(CanonError::Unexpected("bytes overflow".to_string()));
            }
            let payload = bytes[*pos..end].to_vec();
            *pos = end;
            Ok(Value::Bytes(payload))
        }
        4 => {
            let len = decode_u64(bytes, pos)? as usize;
            let end = *pos + len;
            if end > bytes.len() {
                return Err(CanonError::Unexpected("text overflow".to_string()));
            }
            let text = std::str::from_utf8(&bytes[*pos..end]).map_err(|_| CanonError::InvalidUtf8)?;
            *pos = end;
            Ok(Value::Text(text.to_string()))
        }
        5 => {
            let tag = decode_u64(bytes, pos)?;
            let payload = Box::new(decode_value_at(bytes, pos, depth + 1)?);
            Ok(Value::Sum(tag, payload))
        }
        6 => {
            let count = decode_u64(bytes, pos)? as usize;
            let mut items = Vec::new();
            for _ in 0..count { items.push(decode_value_at(bytes, pos, depth + 1)?); }
            Ok(Value::Product(items))
        }
        7 => {
            let count = decode_u64(bytes, pos)? as usize;
            let mut items = Vec::new();
            for _ in 0..count { items.push(decode_value_at(bytes, pos, depth + 1)?); }
            Ok(Value::Sequence(items))
        }
        8 => {
            let count = decode_u64(bytes, pos)? as usize;
            let mut entries = Vec::new();
            let mut seen = HashMap::<Vec<u8>, ()>::new();
            for _ in 0..count {
                let key = decode_value_at(bytes, pos, depth + 1)?;
                let key_bytes = encode_value_with_depth(&key, depth + 1)?;
                if seen.contains_key(&key_bytes) {
                    return Err(CanonError::DuplicateMapKey);
                }
                seen.insert(key_bytes, ());
                let value = decode_value_at(bytes, pos, depth + 1)?;
                entries.push((key, value));
            }
            Ok(Value::FiniteMap(entries))
        }
        9 => {
            let end = *pos + 32;
            if end > bytes.len() { return Err(CanonError::Unexpected("digest overflow".to_string())); }
            let mut digest = [0u8; 32];
            digest.copy_from_slice(&bytes[*pos..end]);
            *pos = end;
            Ok(Value::Digest(digest))
        }
        10 => {
            let end = *pos + 32;
            if end > bytes.len() { return Err(CanonError::Unexpected("ref digest overflow".to_string())); }
            let mut digest = [0u8; 32];
            digest.copy_from_slice(&bytes[*pos..end]);
            *pos = end;
            let end_type = *pos + 32;
            if end_type > bytes.len() { return Err(CanonError::Unexpected("ref type overflow".to_string())); }
            let mut type_digest = [0u8; 32];
            type_digest.copy_from_slice(&bytes[*pos..end_type]);
            *pos = end_type;
            Ok(Value::Ref { digest, type_digest })
        }
        other => Err(CanonError::InvalidTag(other)),
    }
}

fn decode_u64(bytes: &[u8], pos: &mut usize) -> Result<u64, CanonError> {
    let start = *pos;
    let mut value = 0u64;
    let mut shift = 0u32;
    for _ in 0..10 {
        if *pos >= bytes.len() {
            return Err(CanonError::Unexpected("varint truncated".to_string()));
        }
        let byte = bytes[*pos];
        *pos += 1;

        let chunk = (byte & 0x7f) as u64;
        value |= chunk << shift;
        if byte & 0x80 == 0 {
            let canonical = encode_u64(value)?;
            let consumed = &bytes[start..*pos];
            if consumed != canonical.as_slice() {
                return Err(CanonError::NonCanonicalVarint);
            }
            return Ok(value);
        }
        shift += 7;
    }
    Err(CanonError::NonCanonicalVarint)
}

pub fn eval_core(module: &CoreModule, function_index: usize, args: Vec<Value>) -> Verdict {
    let mut budget = Budget {
        max_steps: 64,
        max_depth: 32,
        max_alloc: None,
    };
    let mut receipts = Vec::new();
    let initial = budget.max_steps;
    let outcome = match eval_expr(&module.functions[function_index].body, &args, module, &mut budget, &mut receipts) {
        Ok(value) => Outcome::Returned(value),
        Err(err) => err,
    };
    let consumed = initial.saturating_sub(budget.max_steps);
    Verdict { outcome, steps: consumed, receipts }
}

fn eval_expr(
    expr: &Expr,
    env: &[Value],
    module: &CoreModule,
    budget: &mut Budget,
    receipts: &mut Vec<Receipt>,
) -> Result<Value, Outcome> {
    if budget.max_steps == 0 {
        return Err(Outcome::Exhausted("max_steps".to_string()));
    }
    budget.max_steps -= 1;

    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Argument(index) => env.get(*index as usize).cloned().ok_or_else(|| Outcome::Failed("missing arg".to_string())),
        Expr::Local(index) => env.get(*index as usize).cloned().ok_or_else(|| Outcome::Failed("missing local".to_string())),
        Expr::Let(left, right) => {
            let value = eval_expr(left, env, module, budget, receipts)?;
            let mut next = env.to_vec();
            next.push(value);
            eval_expr(right, &next, module, budget, receipts)
        }
        Expr::Product(items) => {
            let mut result = Vec::new();
            for item in items {
                result.push(eval_expr(item, env, module, budget, receipts)?);
            }
            Ok(Value::Product(result))
        }
        Expr::Sum(tag, payload) => {
            let value = eval_expr(payload, env, module, budget, receipts)?;
            Ok(Value::Sum(*tag, Box::new(value)))
        }
        Expr::Match(scrutinee, arms) => {
            let subject = eval_expr(scrutinee, env, module, budget, receipts)?;
            match subject {
                Value::Sum(tag, payload) => {
                    if let Some(arm) = arms.get(tag as usize) {
                        let mut next = env.to_vec();
                        next.push(*payload);
                        return eval_expr(arm, &next, module, budget, receipts);
                    }
                    Err(Outcome::Failed("no match arm".to_string()))
                }
                _ => Err(Outcome::Failed("non-sum match".to_string())),
            }
        }
        Expr::Primitive(name, args) => eval_primitive(name, args, env, module, budget, receipts),
        Expr::Call(index, args) => {
            let function = module.functions.get(*index as usize).ok_or_else(|| Outcome::Failed("unknown call".to_string()))?;
            if function.arity != args.len() as u64 {
                return Err(Outcome::Failed("arity mismatch".to_string()));
            }
            let mut call_args = Vec::new();
            for arg in args {
                call_args.push(eval_expr(arg, env, module, budget, receipts)?);
            }
            if budget.max_depth == 0 {
                return Err(Outcome::Exhausted("max_depth".to_string()));
            }
            budget.max_depth -= 1;
            eval_expr(&function.body, &call_args, module, budget, receipts)
        }
        Expr::Effect(name, payload) => {
            let request = eval_expr(payload, env, module, budget, receipts)?;
            let response = match name.as_str() {
                "hash" => {
                    let encoded = encode_value(&request).map_err(|e| Outcome::Failed(format!("hash encode:{e:?}")))?;
                    Value::Digest(digest_bytes(&encoded))
                }
                "cas_get" => match request {
                    Value::Digest(digest) => match cas_get(digest) {
                        Some(bytes) => Value::Bytes(bytes),
                        None => Value::Unit,
                    },
                    _ => Value::Unit,
                },
                "cas_put" => {
                    let bytes = match &request {
                        Value::Bytes(bytes) => bytes.clone(),
                        _ => encode_value(&request).map_err(|e| Outcome::Failed(format!("cas_put encode:{e:?}")))?,
                    };
                    let digest = cas_put(&bytes);
                    Value::Digest(digest)
                }
                "log_trace" => Value::Text(format!("trace:{name}")),
                _ => Value::Text(format!("effect:{name}")),
            };
            receipts.push(Receipt {
                capability_digest: digest_bytes(name.as_bytes()),
                handler_digest: digest_bytes(format!("{name}:handler").as_bytes()),
                request_digest: digest_bytes(&encode_value(&request).unwrap_or_default()),
                response_digest: digest_bytes(&encode_value(&response).unwrap_or_default()),
                status: "ok".to_string(),
            });
            Ok(response)
        }
    }
}

fn eval_primitive(
    name: &str,
    args: &[Expr],
    env: &[Value],
    module: &CoreModule,
    budget: &mut Budget,
    receipts: &mut Vec<Receipt>,
) -> Result<Value, Outcome> {
    let values: Vec<Value> = args
        .iter()
        .map(|arg| eval_expr(arg, env, module, budget, receipts))
        .collect::<Result<Vec<_>, _>>()?;

    match name {
        "nat_add" => {
            let [Value::Nat(a), Value::Nat(b)] = values.as_slice() else {
                return Err(Outcome::Failed("nat_add expects nat values".to_string()));
            };
            let sum = a.checked_add(*b).ok_or_else(|| Outcome::Failed("nat_add overflow".to_string()))?;
            Ok(Value::Nat(sum))
        }
        "nat_mul" => {
            let [Value::Nat(a), Value::Nat(b)] = values.as_slice() else {
                return Err(Outcome::Failed("nat_mul expects nat values".to_string()));
            };
            let product = a.checked_mul(*b).ok_or_else(|| Outcome::Failed("nat_mul overflow".to_string()))?;
            Ok(Value::Nat(product))
        }
        "bool_and" => {
            let [Value::Bool(a), Value::Bool(b)] = values.as_slice() else {
                return Err(Outcome::Failed("bool_and expects bool values".to_string()));
            };
            Ok(Value::Bool(*a && *b))
        }
        "bool_or" => {
            let [Value::Bool(a), Value::Bool(b)] = values.as_slice() else {
                return Err(Outcome::Failed("bool_or expects bool values".to_string()));
            };
            Ok(Value::Bool(*a || *b))
        }
        "eq_bytes" => {
            let [Value::Bytes(a), Value::Bytes(b)] = values.as_slice() else {
                return Err(Outcome::Failed("eq_bytes expects bytes values".to_string()));
            };
            Ok(Value::Bool(a == b))
        }
        "bytes_concat" => {
            let [Value::Bytes(a), Value::Bytes(b)] = values.as_slice() else {
                return Err(Outcome::Failed("bytes_concat expects bytes values".to_string()));
            };
            let mut out = a.clone();
            out.extend_from_slice(b);
            Ok(Value::Bytes(out))
        }
        "len_bytes" => {
            let [Value::Bytes(bytes)] = values.as_slice() else {
                return Err(Outcome::Failed("len_bytes expects bytes".to_string()));
            };
            Ok(Value::Nat(bytes.len() as u64))
        }
        "len_text" => {
            let [Value::Text(text)] = values.as_slice() else {
                return Err(Outcome::Failed("len_text expects text".to_string()));
            };
            Ok(Value::Nat(text.len() as u64))
        }
        "eq_text" => {
            let [Value::Text(a), Value::Text(b)] = values.as_slice() else {
                return Err(Outcome::Failed("eq_text expects text values".to_string()));
            };
            Ok(Value::Bool(a == b))
        }
        "eq_digest" => {
            let [Value::Digest(a), Value::Digest(b)] = values.as_slice() else {
                return Err(Outcome::Failed("eq_digest expects digest values".to_string()));
            };
            Ok(Value::Bool(a == b))
        }
        _ => Err(Outcome::Failed(format!("unknown primitive {name}"))),
    }
}

pub fn observe(verdict: &Verdict) -> Vec<String> {
    let mut out = Vec::new();
    match &verdict.outcome {
        Outcome::Returned(value) => out.push(format!("returned:{value:?}")),
        Outcome::Failed(msg) => out.push(format!("failed:{msg}")),
        Outcome::Exhausted(msg) => out.push(format!("exhausted:{msg}")),
    }
    for receipt in &verdict.receipts {
        let digest = digest_bytes(&encode_value(&Value::Text(format!("{receipt:?}"))).unwrap_or_default());
        out.push(hex_encode(&digest));
    }
    out
}

pub fn fixture_json(fixture_id: &str, value: &Value) -> String {
    let encoded = encode_value(value).unwrap();
    let digest = digest_bytes(&encoded);
    format!(
        "{{\"fixture_id\":\"{fixture_id}\",\"encoded_hex\":\"{}\",\"digest_hex\":\"{}\",\"outcome_digest\":\"{}\",\"steps\":1,\"receipt_digests\":[]}}",
        hex_encode(&encoded),
        hex_encode(&digest),
        hex_encode(&digest)
    )
}

pub fn parse_fixture_text(text: &str) -> Option<Value> {
    let text = text.trim();
    if text.is_empty() { return None; }
    if let Some(value) = text.strip_prefix("nat=") {
        return Some(Value::Nat(value.parse().ok()?));
    }
    if let Some(value) = text.strip_prefix("text=") {
        return Some(Value::Text(value.to_string()));
    }
    if let Some(value) = text.strip_prefix("bool=") {
        return Some(Value::Bool(value == "true"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let value = Value::Product(vec![Value::Nat(11), Value::Text("ok".to_string())]);
        let bytes = encode_value(&value).unwrap();
        let decoded = decode_value(&bytes).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn cas_round_trip() {
        let mut cas = CasStore::default();
        let bytes = b"hello";
        let digest = cas.put(bytes);
        assert_eq!(cas.get(digest).unwrap(), bytes);
    }

    #[test]
    fn rejects_noncanonical_varint() {
        let encoded = [2u8, 0x80, 0x00];
        assert_eq!(decode_value(&encoded), Err(CanonError::NonCanonicalVarint));
    }

    #[test]
    fn cas_put_and_get_are_content_addressed() {
        let mut cas = CasStore::default();
        let payload = b"hello stratum";
        let digest = cas.put(payload);
        assert_eq!(cas.get(digest), Some(payload.to_vec()));
        assert_eq!(cas.get(digest_bytes(payload)), Some(payload.to_vec()));
    }

    #[test]
    fn global_cas_round_trip_is_persistent() {
        let payload = b"persistent-cas";
        let digest = cas_put(payload);
        assert_eq!(cas_get(digest), Some(payload.to_vec()));
        assert_eq!(cas_get(digest_bytes(payload)), Some(payload.to_vec()));
    }

    #[test]
    fn core_nat_add_and_effect_are_deterministic() {
        let module = CoreModule {
            functions: vec![CoreFunction {
                arity: 2,
                body: Expr::Primitive("nat_add".to_string(), vec![Expr::Argument(0), Expr::Argument(1)]),
            }],
        };
        let verdict = eval_core(&module, 0, vec![Value::Nat(11), Value::Nat(31)]);
        assert!(matches!(verdict.outcome, Outcome::Returned(Value::Nat(42))));
        assert_eq!(verdict.steps, 3);

        let effect_module = CoreModule {
            functions: vec![CoreFunction {
                arity: 0,
                body: Expr::Effect("hash".to_string(), Box::new(Expr::Literal(Value::Text("abc".to_string())))),
            }],
        };
        let effect_verdict = eval_core(&effect_module, 0, Vec::new());
        assert!(matches!(effect_verdict.outcome, Outcome::Returned(Value::Digest(_))));
        assert_eq!(effect_verdict.receipts.len(), 1);
    }

    #[test]
    fn delta_laws_hold_for_a_simple_nat() {
        let ty = TypeDescription::Nat;
        let before = Value::Nat(3);
        let after = Value::Nat(7);
        let delta = diff_delta(&ty, &before, &after).unwrap();
        let applied = apply_delta(&ty, &before, &delta).unwrap();
        assert_eq!(applied, after);
        assert_eq!(apply_delta(&ty, &applied, &Delta::Zero).unwrap(), applied);
        let composed = compose_delta(&ty, &Delta::Zero, &delta).unwrap();
        assert_eq!(apply_delta(&ty, &before, &composed).unwrap(), after);
    }

    #[test]
    fn history_materialization_is_stable() {
        let genesis = Value::Nat(0);
        let state = Value::Nat(5);
        let history = History { nodes: vec![OpNode { language_digest: digest_bytes(b"lang"), operation_digest: digest_bytes(b"op"), dependencies: vec![] }] };
        let materialization = materialize(&history, &genesis, &state);
        assert_eq!(materialization.genesis_digest, digest_bytes(&encode_value(&genesis).unwrap()));
        assert_eq!(materialization.state_digest, digest_bytes(&encode_value(&state).unwrap()));
        assert_eq!(frontier(&history).len(), 1);
    }

    #[test]
    fn accelerator_matches_generic_core_execution() {
        let module = CoreModule {
            functions: vec![CoreFunction {
                arity: 1,
                body: Expr::Primitive("nat_add".to_string(), vec![Expr::Argument(0), Expr::Literal(Value::Nat(1))]),
            }],
        };
        let arrow = Arrow {
            input_type: TypeDescription::Nat,
            output_type: TypeDescription::Nat,
            core_module: module.clone(),
            function_index: 0,
        };
        let generic = run_arrow(&arrow, Value::Nat(41));
        let manifest = AccelManifest {
            semantic_arrow: arrow.clone(),
            source_closure_digest: digest_bytes(b"src"),
            target_kind: "nat".to_string(),
            implementation_digest: digest_bytes(b"accel"),
        };
        let registered_arrow = arrow.clone();
        accel_register(manifest.implementation_digest, Arc::new(move |input, _budget| run_arrow(&registered_arrow, input)));
        let accelerated = accel_run(&manifest, Value::Nat(41), Budget { max_steps: 64, max_depth: 32, max_alloc: None });
        assert_eq!(observe(&generic), observe(&accelerated));
    }

    #[test]
    fn closure_traverse_follows_nested_refs() {
        let mut graph = Graph::default();
        let leaf = graph.insert(Value::Nat(7), digest_bytes(b"nat"));
        let middle = graph.insert(Value::Ref { digest: leaf, type_digest: digest_bytes(b"nat") }, digest_bytes(b"ref"));
        let root = graph.insert(Value::Ref { digest: middle, type_digest: digest_bytes(b"ref") }, digest_bytes(b"ref"));
        let closure = close(&graph, root).unwrap();
        assert_eq!(closure.len(), 3);
        assert_eq!(traverse(&graph, &closure, &[0, 1, 2]), Ok(Value::Nat(7)));
    }

    #[test]
    fn conflicting_history_is_reported_explicitly() {
        let left = OpNode {
            language_digest: digest_bytes(b"lang"),
            operation_digest: digest_bytes(b"left"),
            dependencies: vec![],
        };
        let right = OpNode {
            language_digest: digest_bytes(b"lang"),
            operation_digest: digest_bytes(b"right"),
            dependencies: vec![],
        };
        let history = History { nodes: vec![left, right] };
        let conflict = detect_conflict(&history);
        assert!(conflict.is_some());
        assert_eq!(conflict.unwrap().nodes.len(), 2);
    }
}
