#![cfg(feature = "merge")]

use callback_data::MergeTo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Data1 {
    user_id: i64,
    foo: String,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Data2 {
    user_id: i64,
    value: i32,
}

#[test]
fn merge_to_takes_matching_field_and_extra_fields() {
    let d1 = Data1 {
        user_id: 5,
        foo: "unused".into(),
    };

    let d2: Data2 = d1
        .merge_to(callback_data::fields! { value: 1 })
        .expect("merge should succeed");

    assert_eq!(
        d2,
        Data2 {
            user_id: 5,
            value: 1
        }
    );
}

#[test]
fn merge_associated_fn_style_matches_merge_to() {
    use callback_data::MergeFrom;

    let d1 = Data1 {
        user_id: 5,
        foo: "unused".into(),
    };

    let d2 = Data2::merge(&d1, callback_data::fields! { value: 1 }).expect("merge should succeed");

    assert_eq!(
        d2,
        Data2 {
            user_id: 5,
            value: 1
        }
    );
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Data3 {
    user_id: i64,
    value: i32,
    cancel: bool,
}

#[test]
fn missing_field_falls_back_to_target_default_instead_of_erroring() {
    let d1 = Data1 {
        user_id: 5,
        foo: "unused".into(),
    };

    // `cancel` is present on neither Data1 nor the extra fields — it must
    // come from Data3::default() (false), not error out.
    let d3: Data3 = d1
        .merge_to(callback_data::fields! { value: 1 })
        .expect("merge should fall back to Target::default() for missing fields");

    assert_eq!(
        d3,
        Data3 {
            user_id: 5,
            value: 1,
            cancel: false,
        }
    );
}
