//! Dict and list building operations (design 5.4, 6.5.3).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::value::key::Key;
use crate::value::value::{ListVal, StructVal, Value};
use crate::vm::fail::{FailCode, Failure};

fn type_err(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

/// Splits a pair (a 2-list with a key-typed head) into key and value.
fn as_pair(v: &Value) -> Result<(Key, Value), Failure> {
    match v {
        Value::List(l) => match &*l.items {
            [k, val] => Ok((Key::from_value(k)?, val.clone())),
            _ => Err(type_err("a dict element must be a [key value] pair")),
        },
        _ => Err(type_err("a dict element must be a [key value] pair")),
    }
}

/// Builds a dict from pair elements. A repeated key keeps the last pair.
///
/// # Errors
/// `Type` when an element is not a pair with a key-typed head.
pub fn dict_from_pairs(items: &[Value]) -> Result<BTreeMap<Key, Value>, Failure> {
    let mut map = BTreeMap::new();
    for item in items {
        let (k, v) = as_pair(item)?;
        map.insert(k, v);
    }
    Ok(map)
}

/// The pairs of a dict as 2-lists, in key order.
pub fn pairs(d: &BTreeMap<Key, Value>) -> impl Iterator<Item = Value> + '_ {
    d.iter()
        .map(|(k, v)| Value::list(vec![k.to_value(), v.clone()]))
}

/// `put`: appends to a list, assocs pairs into a dict (later pairs win; a
/// dict element merges its pairs), or replaces known fields of a struct.
///
/// # Errors
/// `Type` for `nil` or a non-collection, and for a non-pair element on a
/// dict; `UnknownField` for an unknown struct field.
pub fn put(coll: &Value, elems: &[Value]) -> Result<Value, Failure> {
    match coll {
        Value::List(l) => {
            let mut items = Vec::with_capacity(l.items.len() + elems.len());
            items.extend(l.items.iter().cloned());
            items.extend(elems.iter().cloned());
            Ok(Value::list(items))
        }
        Value::Dict(d) => {
            let mut map = (**d).clone();
            for e in elems {
                if let Value::Dict(o) = e {
                    map.extend(o.iter().map(|(k, v)| (k.clone(), v.clone())));
                    continue;
                }
                let (k, v) = as_pair(e)?;
                map.insert(k, v);
            }
            Ok(Value::dict(map))
        }
        Value::Struct(s) => {
            let mut fields = s.fields.to_vec();
            for e in elems {
                let (k, v) = as_pair(e)?;
                let Key::Kw(kw) = k else {
                    return Err(Failure::new(
                        FailCode::UnknownField,
                        "struct fields are keywords",
                    ));
                };
                match fields.iter_mut().find(|(f, _)| *f == kw) {
                    Some(slot) => slot.1 = v,
                    None => {
                        return Err(Failure::new(FailCode::UnknownField, "unknown struct field"))
                    }
                }
            }
            Ok(Value::Struct(Rc::new(StructVal {
                ty: s.ty,
                fields: fields.into_boxed_slice(),
            })))
        }
        Value::Nil => Err(type_err("put on nil")),
        _ => Err(type_err("put expects a list, dict or struct")),
    }
}

/// `join`: the first collection sets the result kind. Dicts merge (the last
/// value wins); lists concatenate, and a dict among them contributes its pairs.
/// An empty input gives the empty list.
///
/// # Errors
/// `Type` for a non-collection, or a non-pair element merged into a dict.
pub fn join(colls: &[Value]) -> Result<Value, Failure> {
    let Some(first) = colls.first() else {
        return Ok(Value::list(Vec::new()));
    };
    match first {
        Value::Dict(d) => {
            let mut map = (**d).clone();
            for c in &colls[1..] {
                match c {
                    Value::Dict(o) => map.extend(o.iter().map(|(k, v)| (k.clone(), v.clone()))),
                    Value::List(l) => {
                        for item in l.items.iter() {
                            let (k, v) = as_pair(item)?;
                            map.insert(k, v);
                        }
                    }
                    _ => return Err(type_err("join of a dict expects dicts or lists of pairs")),
                }
            }
            Ok(Value::dict(map))
        }
        Value::List(_) => {
            let mut items = Vec::new();
            for c in colls {
                match c {
                    Value::List(l) => items.extend(l.items.iter().cloned()),
                    Value::Dict(d) => items.extend(pairs(d)),
                    _ => return Err(type_err("join of a list expects lists or dicts")),
                }
            }
            Ok(Value::List(Rc::new(ListVal {
                items: items.into_boxed_slice(),
                prov: None,
            })))
        }
        _ => Err(type_err("join expects lists or dicts")),
    }
}
