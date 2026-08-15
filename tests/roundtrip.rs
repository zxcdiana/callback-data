use callback_data::{CallbackDataTrait, FlatField, callback_data};

#[derive(Debug, Clone, PartialEq, callback_data::FlatField)]
struct Extended {
    value: i32,
}

#[derive(Debug, Clone, PartialEq, callback_data::FlatField)]
enum Action {
    Ban,
    Kick { reason_code: u32 },
}

#[derive(Debug, Clone, PartialEq)]
#[callback_data(prefix = "adm")]
struct AdminAction {
    #[callback_data(owner)]
    user_id: i64,
    action: Action,
    extended: Extended,
    target: Option<i64>,
}

#[test]
fn pack_unpack_roundtrip_unit_variant_no_target() {
    let data = AdminAction {
        user_id: 1,
        action: Action::Ban,
        extended: Extended { value: 42 },
        target: None,
    };

    let packed = data.pack().expect("pack should succeed");
    let parsed = AdminAction::unpack(&packed).expect("unpack should succeed");

    assert_eq!(data, parsed);
}

#[test]
fn pack_unpack_roundtrip_struct_variant_with_target() {
    let data = AdminAction {
        user_id: 7,
        action: Action::Kick { reason_code: 5 },
        extended: Extended { value: -3 },
        target: Some(999),
    };

    let packed = data.pack().expect("pack should succeed");
    let parsed = AdminAction::unpack(&packed).expect("unpack should succeed");

    assert_eq!(data, parsed);
}

#[test]
fn prefix_is_checked() {
    let data = AdminAction {
        user_id: 1,
        action: Action::Ban,
        extended: Extended { value: 0 },
        target: None,
    };
    let packed = data.pack().unwrap();
    let wrong_prefix = packed.replacen("adm", "xyz", 1);
    assert!(AdminAction::unpack(&wrong_prefix).is_err());
}

#[test]
fn garbage_does_not_panic() {
    assert!(AdminAction::unpack("").is_err());
    assert!(AdminAction::unpack("adm:not_a_number:0:0:~::").is_err());
    assert!(AdminAction::unpack("adm:1:0").is_err()); // too few tokens
}

#[derive(Debug, Clone, PartialEq)]
#[callback_data(prefix = "noown")]
struct NoOwner {
    value: u32,
}

#[test]
fn owner_id_defaults_to_none_without_marked_field() {
    let data = NoOwner { value: 1 };
    assert_eq!(data.owner_id(), None);
}

#[test]
fn owner_id_works() {
    let data = AdminAction {
        user_id: 42,
        action: Action::Ban,
        extended: Extended { value: 0 },
        target: None,
    };
    assert_eq!(data.owner_id(), Some(42));
    assert_ne!(data.owner_id(), Some(1));
}

#[test]
fn token_count_matches_field_layout() {
    // 1 (Action tag) + max(0, 1) [Kick has 1 field] = 2
    assert_eq!(Action::TOKEN_COUNT, 2);
    // Extended: 1 field
    assert_eq!(Extended::TOKEN_COUNT, 1);
    // AdminAction as FlatField (nestable): user_id(1) + action(2) + extended(1) + target(1+i64::TOKEN_COUNT=2) = 6
    assert_eq!(AdminAction::TOKEN_COUNT, 1 + 2 + 1 + 2);
}
