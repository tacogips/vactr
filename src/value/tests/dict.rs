use super::{pair, r, strukt};
use crate::value::{dict_from_pairs, first, join, pairs, put, Value};
use crate::vm::fail::FailCode;

fn dict(items: Vec<Value>) -> Value {
    Value::dict(dict_from_pairs(&items).expect("pairs"))
}

#[test]
fn iteration_in_key_order_for_out_of_order_inserts() {
    let d = dict_from_pairs(&[
        pair(Value::kw("pan"), Value::Int(1)),
        pair(Value::str("z"), Value::Int(2)),
        pair(Value::Int(3), Value::Int(3)),
        pair(Value::kw("amp"), Value::Int(4)),
        pair(Value::Ratio(r(1, 2)), Value::Int(5)),
    ])
    .unwrap();
    let keys: Vec<String> = pairs(&d).map(|p| p.to_string()).collect();
    assert_eq!(
        keys,
        ["[1/2 5]", "[3 3]", "[:amp 4]", "[:pan 1]", "[\"z\" 2]"]
    );
}

#[test]
fn duplicate_literal_key_keeps_last_pair() {
    let d = dict_from_pairs(&[
        pair(Value::kw("amp"), Value::Int(1)),
        pair(Value::kw("pan"), Value::Int(0)),
        pair(Value::kw("amp"), Value::Int(2)),
        pair(Value::Float(1.0), Value::Int(3)),
        pair(Value::Int(1), Value::Int(4)),
    ])
    .unwrap();
    assert_eq!(d.len(), 3);
    assert_eq!(Value::dict(d).to_string(), "[[1 4] amp: 2 pan: 0]");
}

#[test]
fn non_pair_literal_is_type() {
    assert_eq!(
        dict_from_pairs(&[Value::Int(1)]).unwrap_err().code,
        FailCode::Type
    );
    let triple = Value::list(vec![Value::kw("a"), Value::Int(1), Value::Int(2)]);
    assert_eq!(dict_from_pairs(&[triple]).unwrap_err().code, FailCode::Type);
}

#[test]
fn put_on_dict_replaces_a_key() {
    let d = dict(vec![
        pair(Value::kw("amp"), Value::Int(1)),
        pair(Value::kw("pan"), Value::Int(0)),
    ]);
    let out = put(&d, &[pair(Value::kw("amp"), Value::Float(0.5))]).unwrap();
    assert_eq!(out.to_string(), "[amp: 0.5 pan: 0]");
    // The input is unchanged.
    assert_eq!(d.to_string(), "[amp: 1 pan: 0]");
    let out = put(
        &d,
        &[
            pair(Value::kw("x"), Value::Int(1)),
            pair(Value::kw("x"), Value::Int(2)),
        ],
    )
    .unwrap();
    assert_eq!(out.to_string(), "[amp: 1 pan: 0 x: 2]");
}

#[test]
fn put_on_dict_with_non_pair_is_type() {
    let d = dict(vec![pair(Value::kw("amp"), Value::Int(1))]);
    assert_eq!(put(&d, &[Value::Int(1)]).unwrap_err().code, FailCode::Type);
    assert_eq!(
        put(&d, &[pair(Value::list(vec![]), Value::Int(1))])
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

#[test]
fn put_on_list_appends() {
    let l = Value::list(vec![Value::Int(1)]);
    assert_eq!(
        put(&l, &[Value::Int(2), Value::Int(3)])
            .unwrap()
            .to_string(),
        "[1 2 3]"
    );
}

#[test]
fn put_on_nil_is_type() {
    assert_eq!(
        put(&Value::Nil, &[Value::Int(1)]).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(
        put(&Value::Int(1), &[Value::Int(1)]).unwrap_err().code,
        FailCode::Type
    );
}

#[test]
fn put_on_struct_is_closed() {
    let s = strukt("voice", &[("amp", Value::Int(1)), ("pan", Value::Int(0))]);
    let out = put(&s, &[pair(Value::kw("amp"), Value::Int(2))]).unwrap();
    assert_eq!(out.to_string(), "voice amp: 2 pan: 0");
    assert_eq!(
        put(&s, &[pair(Value::kw("nope"), Value::Int(2))])
            .unwrap_err()
            .code,
        FailCode::UnknownField
    );
}

#[test]
fn join_of_lists_concatenates() {
    let a = Value::list(vec![Value::Int(1)]);
    let b = Value::list(vec![Value::Int(2), Value::Int(3)]);
    assert_eq!(join(&[a.clone(), b]).unwrap().to_string(), "[1 2 3]");
    let d = dict(vec![pair(Value::kw("k"), Value::Int(9))]);
    assert_eq!(join(&[a, d]).unwrap().to_string(), "[1 [:k 9]]");
    assert_eq!(join(&[]).unwrap().to_string(), "[]");
}

#[test]
fn join_of_dicts_merges_last_wins() {
    let a = dict(vec![
        pair(Value::kw("amp"), Value::Int(1)),
        pair(Value::kw("pan"), Value::Int(0)),
    ]);
    let b = dict(vec![pair(Value::kw("amp"), Value::Int(2))]);
    let c = Value::list(vec![pair(Value::kw("cut"), Value::Int(3))]);
    assert_eq!(
        join(&[a.clone(), b, c]).unwrap().to_string(),
        "[amp: 2 cut: 3 pan: 0]"
    );
    assert_eq!(join(&[a, Value::Int(1)]).unwrap_err().code, FailCode::Type);
    assert_eq!(join(&[Value::Nil]).unwrap_err().code, FailCode::Type);
}

#[test]
fn first_of_dict_is_smallest_pair() {
    let d = dict(vec![
        pair(Value::kw("pan"), Value::Int(0)),
        pair(Value::kw("amp"), Value::Int(1)),
    ]);
    assert_eq!(first(&d).unwrap().to_string(), "[:amp 1]");
}
