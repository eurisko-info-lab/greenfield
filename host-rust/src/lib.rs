use sha2::{Digest as _, Sha256};
use std::collections::HashMap;

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
    let mut store = CasStore::default();
    store.put(bytes)
}

pub fn cas_get(digest: Digest) -> Option<Vec<u8>> {
    let store = CasStore::default();
    store.get(digest)
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
            eval_expr(&function.body, &call_args, module, budget, receipts)
        }
        Expr::Effect(name, payload) => {
            let request = eval_expr(payload, env, module, budget, receipts)?;
            let response = match name.as_str() {
                "hash" => Value::Digest(digest_bytes(&encode_value(&request).unwrap_or_default())),
                "cas_put" => {
                    let mut store = CasStore::default();
                    let digest = store.put(&encode_value(&request).unwrap_or_default());
                    Value::Digest(digest)
                }
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
            Ok(Value::Nat(a.saturating_add(*b)))
        }
        "bool_and" => {
            let [Value::Bool(a), Value::Bool(b)] = values.as_slice() else {
                return Err(Outcome::Failed("bool_and expects bool values".to_string()));
            };
            Ok(Value::Bool(*a && *b))
        }
        "eq_bytes" => {
            let [Value::Bytes(a), Value::Bytes(b)] = values.as_slice() else {
                return Err(Outcome::Failed("eq_bytes expects bytes values".to_string()));
            };
            Ok(Value::Bool(a == b))
        }
        "len_bytes" => {
            let [Value::Bytes(bytes)] = values.as_slice() else {
                return Err(Outcome::Failed("len_bytes expects bytes".to_string()));
            };
            Ok(Value::Nat(bytes.len() as u64))
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
}
