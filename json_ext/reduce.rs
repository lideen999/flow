use estuary_json as ej;
use itertools::EitherOrBoth;
use serde::{Deserialize, Serialize};
use serde_json as sj;
use std::cmp::Ordering;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "strategy", deny_unknown_fields, rename_all = "camelCase")]
pub enum Strategy {
    Minimize(Minimize),
    Maximize(Maximize),
    Sum(Sum),
    Merge(Merge),
    FirstWriteWins(FirstWriteWins),
    LastWriteWins(LastWriteWins),
}

impl std::convert::TryFrom<&sj::Value> for Strategy {
    type Error = serde_json::Error;

    fn try_from(v: &sj::Value) -> Result<Self, Self::Error> {
        Strategy::deserialize(v)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Minimize {
    key: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Maximize {
    key: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sum {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Merge {
    key: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FirstWriteWins {}

#[derive(Serialize, Deserialize, Debug)]
pub struct LastWriteWins {}

pub trait Reducer {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        created: bool,
        sub: &R,
    ) -> usize
    where
        R: Reducer;
}

impl Reducer for Strategy {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        created: bool,
        sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        match self {
            Strategy::Minimize(m) => m.reduce(at, val, into, created, sub),
            Strategy::Maximize(m) => m.reduce(at, val, into, created, sub),
            Strategy::Sum(m) => m.reduce(at, val, into, created, sub),
            Strategy::Merge(m) => m.reduce(at, val, into, created, sub),
            Strategy::FirstWriteWins(m) => m.reduce(at, val, into, created, sub),
            Strategy::LastWriteWins(m) => m.reduce(at, val, into, created, sub),
        }
    }
}

impl Reducer for FirstWriteWins {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        created: bool,
        _sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        if created {
            *into = val;
            at + count_nodes(into)
        } else {
            at + count_nodes(&val)
        }
    }
}

impl Reducer for LastWriteWins {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        _created: bool,
        _sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        *into = val;
        at + count_nodes(into)
    }
}

impl Reducer for Minimize {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        created: bool,
        _sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        if created || json_cmp_at(&self.key, &val, into) == Ordering::Less {
            *into = val;
            at + count_nodes(into)
        } else {
            at + count_nodes(&val)
        }
    }
}

impl Reducer for Maximize {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        created: bool,
        _sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        if created || json_cmp_at(&self.key, &val, into) == Ordering::Greater {
            *into = val;
            at + count_nodes(into)
        } else {
            at + count_nodes(&val)
        }
    }
}

impl Reducer for Sum {
    fn reduce<R>(
        &self,
        at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        created: bool,
        _sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        match (&*into, &val) {
            (sj::Value::Number(lhs), sj::Value::Number(rhs)) => {
                *into = (ej::Number::from(lhs) + ej::Number::from(rhs)).into();
                at + 1
            }
            (sj::Value::Null, sj::Value::Number(_)) if !created => {
                at + 1 // Leave as null.
            }
            _ => {
                *into = val; // Last write wins.
                at + count_nodes(into)
            }
        }
    }
}

impl Reducer for Merge {
    fn reduce<R>(
        &self,
        mut at: usize,
        val: sj::Value,
        into: &mut sj::Value,
        _created: bool,
        sub: &R,
    ) -> usize
    where
        R: Reducer,
    {
        match (into, val) {
            (into @ sj::Value::Array(_), sj::Value::Array(val)) => {
                // TODO: work-around for "cannot bind by-move and by-ref in the same pattern".
                // https://github.com/rust-lang/rust/issues/68354
                let into = into.as_array_mut().unwrap();
                let prev = std::mem::replace(into, Vec::new());

                into.extend(
                    itertools::merge_join_by(prev.into_iter(), val.into_iter(), |lhs, rhs| {
                        json_cmp_at(&self.key, lhs, rhs)
                    })
                    .map(|eob| match eob {
                        EitherOrBoth::Both(mut into, val) => {
                            at = sub.reduce(at + 1, val, &mut into, false, sub);
                            into
                        }
                        EitherOrBoth::Right(val) => {
                            let mut into = sj::Value::Null;
                            at = sub.reduce(at + 1, val, &mut into, true, sub);
                            into
                        }
                        EitherOrBoth::Left(into) => into,
                    }),
                );

                at
            }
            (into @ sj::Value::Object(_), sj::Value::Object(val)) => {
                // TODO: work-around for "cannot bind by-move and by-ref in the same pattern".
                // https://github.com/rust-lang/rust/issues/68354

                type Map = sj::Map<String, sj::Value>;
                let into = into.as_object_mut().unwrap();
                let into_prev = std::mem::replace(into, Map::new());

                into.extend(
                    itertools::merge_join_by(into_prev.into_iter(), val.into_iter(), |lhs, rhs| {
                        lhs.0.cmp(&rhs.0)
                    })
                    .map(|eob| match eob {
                        EitherOrBoth::Both((prop, mut into), (_, val)) => {
                            at = sub.reduce(at + 1, val, &mut into, false, sub);
                            (prop, into)
                        }
                        EitherOrBoth::Right((prop, val)) => {
                            let mut into = sj::Value::Null;
                            at = sub.reduce(at + 1, val, &mut into, true, sub);
                            (prop, into)
                        }
                        EitherOrBoth::Left(into) => into,
                    }),
                );

                at
            }
            (into, val) => {
                *into = val;
                at + count_nodes(into)
            }
        }
    }
}

/// json_cmp_at evaluates the deep ordering of |lhs| and |rhs| with respect
/// to a composite key, specified as a slice of JSON-Pointers relative to the
/// respective document roots. If the slice of JSON-Pointers is empty, the
/// deep ordering with respect to the roots themselves is returned.
fn json_cmp_at<S>(key_ptrs: &[S], lhs: &sj::Value, rhs: &sj::Value) -> Ordering
where
    S: AsRef<str>,
{
    if key_ptrs.is_empty() {
        ej::json_cmp(lhs, rhs)
    } else {
        key_ptrs
            .iter()
            .map(|ptr| {
                ej::json_cmp(
                    lhs.pointer(ptr.as_ref()).unwrap_or(&sj::Value::Null),
                    rhs.pointer(ptr.as_ref()).unwrap_or(&sj::Value::Null),
                )
            })
            .find(|o| *o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    }
}

fn count_nodes(v: &sj::Value) -> usize {
    match v {
        sj::Value::Bool(_) | sj::Value::Null | sj::Value::String(_) | sj::Value::Number(_) => 1,
        sj::Value::Array(v) => v.iter().fold(1, |c, vv| c + count_nodes(vv)),
        sj::Value::Object(v) => v.iter().fold(1, |c, (_prop, vv)| c + count_nodes(vv)),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn test_node_counting() {
        assert_eq!(count_nodes(&json!(true)), 1);
        assert_eq!(count_nodes(&json!("string")), 1);
        assert_eq!(count_nodes(&json!(1234)), 1);
        assert_eq!(count_nodes(&Value::Null), 1);

        assert_eq!(count_nodes(&json!([])), 1);
        assert_eq!(count_nodes(&json!([2, 3, 4])), 4);
        assert_eq!(count_nodes(&json!([2, [4, 5]])), 5);

        assert_eq!(count_nodes(&json!({})), 1);
        assert_eq!(count_nodes(&json!({"2": 2, "3": 3})), 3);
        assert_eq!(count_nodes(&json!({"2": 2, "3": {"4": 4, "5": 5}})), 5);

        let doc: sj::Value = json!({
            "two": [3, [5, 6], {"eight": 8}],
            "nine": "nine",
            "ten": sj::Value::Null,
            "eleven": true,
        });
        assert_eq!(count_nodes(&doc), 11);
    }

    #[test]
    fn test_pointer_compare_objects() {
        let d1 = &json!({"a": 1, "b": 2, "c": 3});
        let d2 = &json!({"a": 2, "b": 1, "c": 3});

        // No pointers? Deep compare document roots.
        assert_eq!(json_cmp_at(&[] as &[&str], d1, d2), Ordering::Less);
        // Simple key ordering.
        assert_eq!(json_cmp_at(&["/a"], d1, d2), Ordering::Less);
        assert_eq!(json_cmp_at(&["/b"], d1, d2), Ordering::Greater);
        assert_eq!(json_cmp_at(&["/c"], d1, d2), Ordering::Equal);
        // Composite key ordering.
        assert_eq!(json_cmp_at(&["/c", "/a"], d1, d2), Ordering::Less);
        assert_eq!(json_cmp_at(&["/c", "/b"], d1, d2), Ordering::Greater);
        assert_eq!(json_cmp_at(&["/c", "/c"], d1, d2), Ordering::Equal);
        assert_eq!(
            json_cmp_at(&["/c", "/c", "/c", "/a"], d1, d2),
            Ordering::Less
        );
    }

    #[test]
    fn test_pointer_compare_arrays() {
        let d1 = &json!([1, 2, 3]);
        let d2 = &json!([2, 1, 3]);

        // Deep compare of document root.
        assert_eq!(json_cmp_at(&[] as &[&str], d1, d2), Ordering::Less);
        // Simple key ordering.
        assert_eq!(json_cmp_at(&["/0"], d1, d2), Ordering::Less);
        assert_eq!(json_cmp_at(&["/1"], d1, d2), Ordering::Greater);
        assert_eq!(json_cmp_at(&["/2"], d1, d2), Ordering::Equal);
        // Composite key ordering.
        assert_eq!(json_cmp_at(&["/2", "/0"], d1, d2), Ordering::Less);
        assert_eq!(json_cmp_at(&["/2", "/1"], d1, d2), Ordering::Greater);
        assert_eq!(json_cmp_at(&["/2", "/2"], d1, d2), Ordering::Equal);
    }

    #[test]
    fn test_pointer_compare_missing() {
        let d1 = &json!({"a": sj::Value::Null, "c": 3});
        let d2 = &json!({"b": 2});

        assert_eq!(json_cmp_at(&["/does/not/exist"], d1, d2), Ordering::Equal);
        // Key exists at |d1| but not |d2|. |d2| value is implicitly null.
        assert_eq!(json_cmp_at(&["/c"], d1, d2), Ordering::Greater);
        // Key exists at |d2| but not |d1|. |d1| value is implicitly null.
        assert_eq!(json_cmp_at(&["/b"], d1, d2), Ordering::Less);
        // Key exists at |d1| but not |d2|. Both are null (implicit and explicit).
        assert_eq!(json_cmp_at(&["/a"], d1, d2), Ordering::Equal);
    }

    #[test]
    fn test_minimize() {
        let m = Minimize {
            key: vec!["/k".to_owned()],
        };
        let mut into = sj::Value::Null;

        // Takes initial value.
        assert_eq!(m.reduce(0, json!({"k": 3, "d": 1}), &mut into, true, &m), 3);
        assert_eq!(&into, &json!({"k": 3, "d": 1}));
        // Ignores larger key.
        assert_eq!(m.reduce(0, json!({"k": 4}), &mut into, false, &m), 2);
        assert_eq!(&into, &json!({"k": 3, "d": 1}));
        // Updates with smaller key.
        assert_eq!(m.reduce(0, json!({"k": 2, "d": 2}), &mut into, false, &m), 3);
        assert_eq!(&into, &json!({"k": 2, "d": 2}));
    }

    #[test]
    fn test_maximize() {
        let m = Maximize {
            key: vec!["/k".to_owned()],
        };
        let mut into = sj::Value::Null;

        // Takes initial value.
        assert_eq!(m.reduce(0, json!({"k": 3, "d": 1}), &mut into, true, &m), 3);
        assert_eq!(&into, &json!({"k": 3, "d": 1}));
        // Ignores smaller key.
        assert_eq!(m.reduce(0, json!({"k": 2}), &mut into, false, &m), 2);
        assert_eq!(&into, &json!({"k": 3, "d": 1}));
        // Updates with laerger key.
        assert_eq!(m.reduce(0, json!({"k": 4, "d": 2}), &mut into, false, &m), 3);
        assert_eq!(&into, &json!({"k": 4, "d": 2}));
    }

    #[test]
    fn test_sum() {
        let m = Sum {};
        let mut into = sj::Value::Null;

        assert_eq!(m.reduce(0, json!(123), &mut into, true, &m), 1);
        assert_eq!(&into, &json!(123));
        // Add unsigned.
        assert_eq!(m.reduce(0, json!(45), &mut into, false, &m), 1);
        assert_eq!(&into, &json!(168));
        // Add signed.
        assert_eq!(m.reduce(0, json!(-70), &mut into, false, &m), 1);
        assert_eq!(&into, &json!(98));
        // Add float.
        assert_eq!(m.reduce(0, json!(0.1), &mut into, false, &m), 1);
        assert_eq!(&into, &json!(98.1));
        // Number which cannot be represented becomes null.
        assert_eq!(m.reduce(0, json!(std::f64::INFINITY), &mut into, false, &m), 1);
        assert_eq!(&into, &sj::Value::Null);
        // And stays null as further values are added.
        assert_eq!(m.reduce(0, json!(-1), &mut into, false, &m), 1);
        assert_eq!(&into, &sj::Value::Null);
    }
}
