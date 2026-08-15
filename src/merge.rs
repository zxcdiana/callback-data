//! Opt-in, `serde`-based field merging between two different
//! callback-data types. This is intentionally separate from `pack`/`unpack`
//! (which never touch `serde`) — merging by field name is inherently a
//! runtime, stringly-keyed operation, so we don't pretend otherwise or try
//! to make it compile-time checked. Requires the `merge` feature and
//! `Serialize + DeserializeOwned` on top of your usual `FlatField` derives.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("serialization failed: {0}")]
    Serialize(#[source] serde_json::Error),

    #[error("deserialization into target type failed: {0}")]
    Deserialize(#[source] serde_json::Error),

    #[error("serialized value was not a JSON object")]
    NotAnObject,
}

/// Build `Target` by starting from `Target::default()`, overlaying
/// `self`'s fields (by name) on top, then overlaying `extra` on top of
/// that. Fields present in `extra` win over `source`, which wins over
/// `Target::default()`. This matches the old crate's `merge`/`merge_to`
/// behavior: a field `Target` needs that isn't in `source` or `extra`
/// silently falls back to whatever `Target::default()` had for it,
/// instead of failing — there is no compile-time checking here.
pub fn merge<Source, Target, Extra>(source: &Source, extra: Extra) -> Result<Target, MergeError>
where
    Source: Serialize,
    Target: Default + Serialize + DeserializeOwned,
    Extra: IntoIterator<Item = (String, Value)>,
{
    let mut map: Map<String, Value> =
        match serde_json::to_value(Target::default()).map_err(MergeError::Serialize)? {
            Value::Object(m) => m,
            _ => return Err(MergeError::NotAnObject),
        };

    if let Value::Object(source_map) =
        serde_json::to_value(source).map_err(MergeError::Serialize)?
    {
        map.extend(source_map);
    } else {
        return Err(MergeError::NotAnObject);
    }

    map.extend(extra);

    serde_json::from_value(Value::Object(map)).map_err(MergeError::Deserialize)
}

/// `self.merge_to::<Target>(extra)` — same as [`merge`], with `self` as the source.
pub trait MergeTo: Serialize + Sized {
    fn merge_to<Target, Extra>(&self, extra: Extra) -> Result<Target, MergeError>
    where
        Target: Default + Serialize + DeserializeOwned,
        Extra: IntoIterator<Item = (String, Value)>,
    {
        merge(self, extra)
    }
}

impl<T: Serialize> MergeTo for T {}

/// `Target::merge(&source, extra)` — an associated-function-style
/// alternative to [`MergeTo::merge_to`], for when it reads better to name
/// the target type at the call site (e.g. `DayMenuData::merge(callback_data, fields!())`).
/// Same behavior as [`merge`], just called from the other side.
pub trait MergeFrom: Default + Serialize + DeserializeOwned + Sized {
    fn merge<Source, Extra>(source: &Source, extra: Extra) -> Result<Self, MergeError>
    where
        Source: Serialize,
        Extra: IntoIterator<Item = (String, Value)>,
    {
        merge(source, extra)
    }
}

impl<T: Default + Serialize + DeserializeOwned> MergeFrom for T {}

/// Build a `(String, serde_json::Value)` map the same way the old
/// hand-rolled `fields!` macro did — no compile-time field-name/type
/// checking, on purpose (see the design discussion: a fully type-checked
/// generic merge between arbitrary callback-data types would need
/// HList-style reflection, which isn't worth the complexity here).
#[macro_export]
macro_rules! fields {
    ($($key:ident : $value:expr),* $(,)?) => {{
        let mut map = $crate::__private::serde_json::Map::new();
        $(
            map.insert(
                stringify!($key).to_owned(),
                $crate::__private::serde_json::to_value($value).unwrap(),
            );
        )*
        map
    }};
}
