//! Flat, compile-time-checked `callback_data` (de)serialization for
//! Telegram bots — an aiogram-`CallbackData`-flavored crate for Rust.
//!
//! ```ignore
//! use callback_data::{callback_data, CallbackDataTrait};
//!
//! #[callback_data(prefix = "adm")]
//! struct AdminAction {
//!     #[callback_data(owner)]
//!     user_id: i64,
//!     action: Action,      // enum, flattened
//!     target: Option<i64>, // Option<T>, flattened
//! }
//!
//! #[derive(callback_data::FlatField)]
//! enum Action {
//!     Ban,
//!     Kick { reason_code: u32 },
//! }
//!
//! let data = AdminAction { user_id: 1, action: Action::Ban, target: None };
//! let packed = data.pack()?; // "adm:1:0:~::"
//! let parsed = AdminAction::unpack(&packed)?;
//! assert_eq!(parsed.owner_id(), Some(1));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod callback_data;
mod error;
mod flat_field;

#[cfg(feature = "merge")]
mod merge;

#[cfg(feature = "telers")]
mod telers_support;

pub use callback_data::{CallbackDataConfig, CallbackDataTrait};
pub use error::{PackError, UnpackError};
pub use flat_field::FlatField;

/// `#[callback_data(prefix = "...")]` — derive for top-level callback-data
/// structs. Generates `CallbackDataConfig`, `FlatField`, and an explicit
/// `CallbackDataTrait` impl (so it can also be pack/unpack'd directly, or
/// nested if you really want to). If a field is marked
/// `#[callback_data(owner)]`, `owner_id()` is overridden to return it —
/// otherwise it stays `None` (the trait's default).
pub use callback_data_derive::callback_data;

/// `#[derive(FlatField)]` — derive for structs/enums meant to be *nested*
/// inside a `#[callback_data(...)]` type (no prefix). Structs flatten their
/// fields in declaration order; enums flatten to a variant index token
/// followed by that variant's (padded-to-max) field tokens.
pub use callback_data_derive::FlatField;

#[cfg(feature = "merge")]
pub use merge::{MergeError, MergeFrom, MergeTo, merge};

#[cfg(feature = "telers")]
pub use telers_support::{
    CallbackDataButtonExt, CallbackDataFilter, OWNER_ID_CONTEXT_KEY, as_filter,
    try_unpack_from_update,
};

// Used internally by the `fields!` macro so callers don't need `serde_json`
// as a direct dependency themselves.
#[cfg(feature = "merge")]
#[doc(hidden)]
pub mod __private {
    pub use serde_json;
}
