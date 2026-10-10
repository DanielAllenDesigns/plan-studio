//! Keeping what this build does not understand, and refusing to write what
//! cannot be read back.
//!
//! * [`Foreign`]: the keys of a loaded plan that no typed field of this build
//!   holds (a newer build's slots, at any depth). They are found by comparing
//!   the loaded text with what the typed plan serializes to, kept on the
//!   [`Project`](crate::Project) and written back at the same place on the next
//!   save (QA-20). Objects are found again by `id` (or `name`), so editing,
//!   moving or deleting the neighbours never attaches a key to the wrong one.
//! * [`Finite`]: a serializer wrapper that writes `0` for a NaN or an
//!   infinity (JSON has no such number; `serde_json` writes `null`, which no
//!   `f64` field reads back) and counts them (QA-23).
//! * [`finite_or_zero`] and friends: `null` in an `f64` field reads as the
//!   field's default, so a plan already damaged by a NaN still opens.
//! * [`max_id`]: the largest `id` anywhere in a JSON tree, for repairing
//!   `next_id` (QA-22).
//! * [`read_each`]: reads a list one record at a time so a record this build
//!   cannot parse does not take its neighbours with it (QA-28).

use serde::de::DeserializeOwned;
use serde::ser::{
    Error as _, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::cell::Cell;

// ----- foreign keys -----

#[derive(Debug, Clone, PartialEq)]
enum Seg {
    Key(String),
    Id(u64),
    Name(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct Residue {
    path: Vec<Seg>,
    key: String,
    value: Value,
}

/// Keys a newer build wrote that this build has no field for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Foreign(Vec<Residue>);

impl Foreign {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many foreign keys are held.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The keys of `loaded` (the text as read) that `typed` (the plan
    /// serialized again) lacks.
    pub fn capture(loaded: &Value, typed: &Value) -> Foreign {
        let mut out = Vec::new();
        walk(loaded, typed, &mut Vec::new(), &mut out);
        Foreign(out)
    }

    /// Writes the held keys into `plan` (the serialized plan) where their
    /// object still is. A key the plan now has is never overwritten, and a
    /// key whose object was deleted is dropped.
    pub fn apply(&self, plan: &mut Value) {
        for r in &self.0 {
            if let Some(Value::Object(m)) = locate(plan, &r.path) {
                m.entry(r.key.clone()).or_insert_with(|| r.value.clone());
            }
        }
    }
}

fn unique_name(items: &[Value], name: &str) -> bool {
    items
        .iter()
        .filter(|x| x.get("name").and_then(Value::as_str) == Some(name))
        .count()
        == 1
}

fn walk(loaded: &Value, typed: &Value, path: &mut Vec<Seg>, out: &mut Vec<Residue>) {
    if loaded == typed {
        return;
    }
    match (loaded, typed) {
        (Value::Object(l), Value::Object(t)) => {
            for (k, lv) in l {
                match t.get(k) {
                    Some(tv) => {
                        path.push(Seg::Key(k.clone()));
                        walk(lv, tv, path, out);
                        path.pop();
                    }
                    None => out.push(Residue {
                        path: path.clone(),
                        key: k.clone(),
                        value: lv.clone(),
                    }),
                }
            }
        }
        (Value::Array(l), Value::Array(t)) => {
            for (i, lv) in l.iter().enumerate() {
                let by_id = lv.get("id").and_then(Value::as_u64);
                let by_name = lv.get("name").and_then(Value::as_str);
                let (seg, tv) = if let Some(id) = by_id {
                    (
                        Seg::Id(id),
                        t.iter()
                            .find(|x| x.get("id").and_then(Value::as_u64) == Some(id)),
                    )
                } else if let Some(n) = by_name.filter(|n| unique_name(t, n)) {
                    (
                        Seg::Name(n.to_string()),
                        t.iter()
                            .find(|x| x.get("name").and_then(Value::as_str) == Some(n)),
                    )
                } else {
                    (Seg::Index(i), t.get(i))
                };
                if let Some(tv) = tv {
                    path.push(seg);
                    walk(lv, tv, path, out);
                    path.pop();
                }
            }
        }
        _ => {}
    }
}

fn locate<'a>(root: &'a mut Value, path: &[Seg]) -> Option<&'a mut Value> {
    let mut cur = root;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => cur.get_mut(k.as_str())?,
            Seg::Id(id) => cur
                .as_array_mut()?
                .iter_mut()
                .find(|x| x.get("id").and_then(Value::as_u64) == Some(*id))?,
            Seg::Name(n) => cur
                .as_array_mut()?
                .iter_mut()
                .find(|x| x.get("name").and_then(Value::as_str) == Some(n.as_str()))?,
            Seg::Index(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(cur)
}

// ----- ids -----

/// Ids above this are not allocated ones (hashes, stamps).
const ID_LIMIT: u64 = 1 << 40;

/// The largest `id` (an integer under a key named `id`) anywhere in `v`.
pub fn max_id(v: &Value) -> u64 {
    let mut best = 0;
    max_id_into(v, &mut best);
    best
}

fn max_id_into(v: &Value, best: &mut u64) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "id" {
                    if let Some(n) = x.as_u64().filter(|n| *n < ID_LIMIT) {
                        *best = (*best).max(n);
                    }
                }
                max_id_into(x, best);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| max_id_into(x, best)),
        _ => {}
    }
}

/// Every `id` (an integer under a key named `id`) anywhere in `v`.
pub fn ids(v: &Value) -> std::collections::BTreeSet<u64> {
    fn walk(v: &Value, out: &mut std::collections::BTreeSet<u64>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if k == "id" {
                        if let Some(n) = x.as_u64().filter(|n| *n < ID_LIMIT) {
                            out.insert(n);
                        }
                    }
                    walk(x, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = std::collections::BTreeSet::new();
    walk(v, &mut out);
    out
}

// ----- numbers -----

/// `f64` that reads `null` (what a NaN was written as) as 0.
pub fn finite_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(0.0))
}

/// `f64` that reads `null` as a wall thickness of 4.5".
pub fn thickness_or_default<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(crate::model::DEFAULT_INTERIOR_THICKNESS))
}

/// `f64` that reads `null` as the default ceiling height.
pub fn height_or_default<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(crate::model::DEFAULT_CEILING_HEIGHT))
}

/// A serializer that writes a non-finite number as 0 and counts them.
pub struct Finite<'a, S> {
    inner: S,
    count: &'a Cell<usize>,
}

impl<'a, S> Finite<'a, S> {
    pub fn new(inner: S, count: &'a Cell<usize>) -> Self {
        Finite { inner, count }
    }
}

struct Wrap<'a, 'b, T: ?Sized> {
    value: &'b T,
    count: &'a Cell<usize>,
}

impl<T: ?Sized + Serialize> Serialize for Wrap<'_, '_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(Finite::new(s, self.count))
    }
}

/// A compound (sequence, map, struct...) whose members go through [`Finite`].
pub struct FiniteCompound<'a, C> {
    inner: C,
    count: &'a Cell<usize>,
}

macro_rules! forward {
    ($($name:ident($ty:ty);)*) => {$(
        fn $name(self, v: $ty) -> Result<Self::Ok, Self::Error> {
            self.inner.$name(v)
        }
    )*};
}

impl<'a, S: Serializer> Serializer for Finite<'a, S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = FiniteCompound<'a, S::SerializeSeq>;
    type SerializeTuple = FiniteCompound<'a, S::SerializeTuple>;
    type SerializeTupleStruct = FiniteCompound<'a, S::SerializeTupleStruct>;
    type SerializeTupleVariant = FiniteCompound<'a, S::SerializeTupleVariant>;
    type SerializeMap = FiniteCompound<'a, S::SerializeMap>;
    type SerializeStruct = FiniteCompound<'a, S::SerializeStruct>;
    type SerializeStructVariant = FiniteCompound<'a, S::SerializeStructVariant>;

    forward! {
        serialize_bool(bool);
        serialize_i8(i8);
        serialize_i16(i16);
        serialize_i32(i32);
        serialize_i64(i64);
        serialize_i128(i128);
        serialize_u8(u8);
        serialize_u16(u16);
        serialize_u32(u32);
        serialize_u64(u64);
        serialize_u128(u128);
        serialize_char(char);
        serialize_str(&str);
        serialize_bytes(&[u8]);
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        if v.is_finite() {
            self.inner.serialize_f32(v)
        } else {
            self.count.set(self.count.get() + 1);
            self.inner.serialize_f32(0.0)
        }
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        if v.is_finite() {
            self.inner.serialize_f64(v)
        } else {
            self.count.set(self.count.get() + 1);
            self.inner.serialize_f64(0.0)
        }
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_some(&Wrap {
            value: v,
            count: self.count,
        })
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_variant(name, index, variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        v: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_newtype_struct(
            name,
            &Wrap {
                value: v,
                count: self.count,
            },
        )
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        v: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_newtype_variant(
            name,
            index,
            variant,
            &Wrap {
                value: v,
                count: self.count,
            },
        )
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(FiniteCompound {
            inner: self.inner.serialize_seq(len)?,
            count: self.count,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(FiniteCompound {
            inner: self.inner.serialize_tuple(len)?,
            count: self.count,
        })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(FiniteCompound {
            inner: self.inner.serialize_tuple_struct(name, len)?,
            count: self.count,
        })
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(FiniteCompound {
            inner: self
                .inner
                .serialize_tuple_variant(name, index, variant, len)?,
            count: self.count,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(FiniteCompound {
            inner: self.inner.serialize_map(len)?,
            count: self.count,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(FiniteCompound {
            inner: self.inner.serialize_struct(name, len)?,
            count: self.count,
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(FiniteCompound {
            inner: self
                .inner
                .serialize_struct_variant(name, index, variant, len)?,
            count: self.count,
        })
    }

    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

impl<C: SerializeSeq> SerializeSeq for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.inner.serialize_element(&Wrap {
            value: v,
            count: self.count,
        })
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTuple> SerializeTuple for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.inner.serialize_element(&Wrap {
            value: v,
            count: self.count,
        })
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTupleStruct> SerializeTupleStruct for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(&Wrap {
            value: v,
            count: self.count,
        })
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeTupleVariant> SerializeTupleVariant for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.inner.serialize_field(&Wrap {
            value: v,
            count: self.count,
        })
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeMap> SerializeMap for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, k: &T) -> Result<(), Self::Error> {
        self.inner.serialize_key(&Wrap {
            value: k,
            count: self.count,
        })
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.inner.serialize_value(&Wrap {
            value: v,
            count: self.count,
        })
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeStruct> SerializeStruct for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Self::Error> {
        self.inner.serialize_field(
            key,
            &Wrap {
                value: v,
                count: self.count,
            },
        )
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C: SerializeStructVariant> SerializeStructVariant for FiniteCompound<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Self::Error> {
        self.inner.serialize_field(
            key,
            &Wrap {
                value: v,
                count: self.count,
            },
        )
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

/// `value` as a JSON tree with every non-finite number written as 0, and how
/// many there were.
pub fn to_value_finite<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<(Value, usize)> {
    let count = Cell::new(0);
    let v = value.serialize(Finite::new(serde_json::value::Serializer, &count))?;
    Ok((v, count.get()))
}

/// `value` as pretty JSON text with every non-finite number written as 0.
pub fn to_pretty_finite<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<String> {
    let count = Cell::new(0);
    let mut buf = Vec::new();
    {
        let mut ser = serde_json::Serializer::pretty(&mut buf);
        value.serialize(Finite::new(&mut ser, &count))?;
    }
    String::from_utf8(buf).map_err(serde_json::Error::custom)
}

/// How many non-finite numbers `value` holds.
pub fn count_non_finite<T: Serialize + ?Sized>(value: &T) -> usize {
    let count = Cell::new(0);
    let _ = value.serialize(Finite::new(serde_json::value::Serializer, &count));
    count.get()
}

// ----- lists read record by record -----

/// What [`read_each`] could not read: the raw records, in order, with their
/// positions in the list.
pub type Unreadable = Vec<(usize, Value)>;

/// Reads each record of a JSON list on its own. A record that does not parse
/// as `T` is returned raw (with its position) instead of failing the list.
pub fn read_each<T: DeserializeOwned>(list: &[Value]) -> (Vec<T>, Unreadable) {
    let mut good = Vec::with_capacity(list.len());
    let mut bad = Vec::new();
    for (i, v) in list.iter().enumerate() {
        match T::deserialize(v) {
            Ok(t) => good.push(t),
            Err(_) => bad.push((i, v.clone())),
        }
    }
    (good, bad)
}

// ----- layers of record lists -----

/// The record lists of `v` (an object of arrays, such as a foundation or
/// schedule layer) cut into the records `L` can read and the ones it cannot.
/// `L` must accept any subset of its lists (`#[serde(default)]`). Returns the
/// readable part as JSON and the unreadable records with the list they came
/// from. A value that is not an object has no records to split.
pub fn split_unreadable<L: DeserializeOwned>(v: &Value) -> (Value, Vec<(String, Value)>) {
    let Value::Object(map) = v else {
        return (v.clone(), Vec::new());
    };
    let mut clean = serde_json::Map::new();
    let mut bad = Vec::new();
    for (key, val) in map {
        let Value::Array(items) = val else {
            clean.insert(key.clone(), val.clone());
            continue;
        };
        let mut keep = Vec::with_capacity(items.len());
        for item in items {
            let probe = serde_json::json!({ key.as_str(): [item] });
            if L::deserialize(&probe).is_ok() {
                keep.push(item.clone());
            } else {
                bad.push((key.clone(), item.clone()));
            }
        }
        clean.insert(key.clone(), Value::Array(keep));
    }
    (Value::Object(clean), bad)
}

/// Reads a layer of record lists. A record that does not parse is left out
/// instead of making the whole layer empty (QA-28); [`keep_unreadable`] puts
/// it back when the layer is stored.
pub fn read_layer<L: DeserializeOwned + Default>(v: &Value) -> L {
    match L::deserialize(v) {
        Ok(l) => l,
        Err(_) => {
            let (clean, _) = split_unreadable::<L>(v);
            L::deserialize(&clean).unwrap_or_default()
        }
    }
}

/// `new` (the layer being stored, as JSON) with the records of `old` (what the
/// slot held) that `L` cannot read added back to their lists. Returns whether
/// anything was added.
pub fn keep_unreadable<L: DeserializeOwned>(old: Option<&Value>, new: &mut Value) -> bool {
    let Some(old) = old else { return false };
    if L::deserialize(old).is_ok() {
        return false;
    }
    let (_, bad) = split_unreadable::<L>(old);
    if bad.is_empty() {
        return false;
    }
    if !new.is_object() {
        *new = Value::Object(serde_json::Map::new());
    }
    if let Value::Object(map) = new {
        for (key, record) in bad.iter().cloned() {
            match map.entry(key).or_insert_with(|| Value::Array(Vec::new())) {
                Value::Array(list) => list.push(record),
                other => *other = Value::Array(vec![record]),
            }
        }
    }
    true
}

/// The slot value for storing `layer`: its JSON (non-finite numbers as 0)
/// with the unreadable records of the slot's `old` value added back; `None`
/// when the layer is empty and nothing had to be kept. On the impossible
/// serialization error the old value stays.
pub fn layer_slot<L: serde::Serialize + DeserializeOwned>(
    layer: &L,
    empty: bool,
    old: Option<&Value>,
) -> Option<Value> {
    let Ok((mut v, _)) = to_value_finite(layer) else {
        return old.cloned();
    };
    let kept = keep_unreadable::<L>(old, &mut v);
    if empty && !kept {
        None
    } else {
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::json;

    #[test]
    fn foreign_keys_follow_their_object_by_id() {
        let loaded =
            json!({"a": 1, "z": 9, "items": [{"id": 1, "x": 1, "f": "keep"}, {"id": 2, "x": 2}]});
        let typed = json!({"a": 1, "items": [{"id": 1, "x": 1}, {"id": 2, "x": 2}]});
        let f = Foreign::capture(&loaded, &typed);
        assert_eq!(f.len(), 2);
        // The first item moved to the end and a new one came in front.
        let mut now = json!({"a": 1, "items": [{"id": 7}, {"id": 2, "x": 5}, {"id": 1, "x": 1}]});
        f.apply(&mut now);
        assert_eq!(now["z"], 9);
        assert_eq!(now["items"][2]["f"], "keep");
        assert!(now["items"][0].get("f").is_none());
        // Deleting the object drops its key.
        let mut gone = json!({"a": 1, "items": [{"id": 2}]});
        f.apply(&mut gone);
        assert!(gone["items"][0].get("f").is_none());
    }

    #[test]
    fn a_key_the_plan_has_again_is_not_overwritten() {
        let f = Foreign::capture(&json!({"k": 1}), &json!({}));
        let mut now = json!({"k": 2});
        f.apply(&mut now);
        assert_eq!(now["k"], 2);
    }

    #[test]
    fn max_id_ignores_counters_and_hashes() {
        let v = json!({"next_id": 99, "walls": [{"id": 7}], "x": {"id": 12}, "h": {"id": 9_000_000_000_000_000u64}});
        assert_eq!(max_id(&v), 12);
    }

    #[test]
    fn non_finite_numbers_are_written_as_zero_and_counted() {
        #[derive(Serialize)]
        struct T {
            a: f64,
            b: Vec<f64>,
            c: Option<f32>,
            d: (f64, f64),
        }
        let t = T {
            a: f64::NAN,
            b: vec![1.5, f64::INFINITY],
            c: Some(f32::NEG_INFINITY),
            d: (2.0, f64::NAN),
        };
        assert_eq!(count_non_finite(&t), 4);
        let (v, n) = to_value_finite(&t).unwrap();
        assert_eq!(n, 4);
        assert_eq!(
            v,
            json!({"a": 0.0, "b": [1.5, 0.0], "c": 0.0, "d": [2.0, 0.0]})
        );
        let text = to_pretty_finite(&t).unwrap();
        assert!(!text.contains("null"));
    }

    #[derive(Debug, Default, PartialEq, Deserialize)]
    #[serde(default)]
    struct Lists {
        a: Vec<u32>,
        b: Vec<String>,
    }

    #[test]
    fn a_layer_keeps_the_records_it_cannot_read() {
        let v = json!({"a": [1, "x", 3], "b": ["p", 7]});
        let l: Lists = read_layer(&v);
        assert_eq!(
            l,
            Lists {
                a: vec![1, 3],
                b: vec!["p".into()]
            }
        );
        let (_, bad) = split_unreadable::<Lists>(&v);
        assert_eq!(
            bad,
            vec![("a".to_string(), json!("x")), ("b".to_string(), json!(7))]
        );
        // Storing the edited layer puts them back.
        let mut new = json!({"a": [1, 3, 9], "b": ["p"]});
        assert!(keep_unreadable::<Lists>(Some(&v), &mut new));
        assert_eq!(new, json!({"a": [1, 3, 9, "x"], "b": ["p", 7]}));
        // A readable slot has nothing to add back.
        let mut same = json!({"a": [1]});
        assert!(!keep_unreadable::<Lists>(
            Some(&json!({"a": [1]})),
            &mut same
        ));
    }

    #[test]
    fn a_list_is_read_record_by_record() {
        let list = vec![json!(1), json!("x"), json!(3)];
        let (good, bad) = read_each::<u32>(&list);
        assert_eq!(good, vec![1, 3]);
        assert_eq!(bad, vec![(1, json!("x"))]);
    }
}
