//! Optional `telers` integration, enabled via the `telers` feature.
//!
//! Deliberately minimal: this module only knows how to (a) turn `Self`
//! into an `InlineKeyboardButton` and (b) act as a `telers::Filter` that
//! unpacks a matching `callback_data` into the context. It has **no**
//! opinion about ownership checks, localized rejection messages, alert
//! flags, or cache times.
//!
//! Ownership: on every successful match, the filter writes
//! `Option<i64>` — [`CallbackDataTrait::owner_id`] — into the context under
//! the fixed key [`OWNER_ID_CONTEXT_KEY`], regardless of whether this
//! particular `T` has an owner field (in which case it writes `None`).
//! This is what lets a single global middleware compare "the owner of
//! whatever callback data matched, if any" against `call.from.id` without
//! needing to know the concrete `T` — and it's always overwritten on every
//! `check()`, so it never carries a stale value from a previous filter in
//! the chain:
//!
//! ```ignore
//! // in some outer middleware, registered once, agnostic of any specific T:
//! if let Some(Some(owner)) = request.context.get::<Option<i64>>(callback_data::OWNER_ID_CONTEXT_KEY) {
//!     if *owner != call.from.id {
//!         // your response, your text, your i18n — this crate has no opinion
//!     }
//! }
//! ```
//!
//! NOTE: `telers::Filter<Client>` is generic over the HTTP client type
//! (default `Reqwest`). Verify the exact trait signature (and `Context`'s
//! `get`/`insert` signatures) against the `telers` version you depend on —
//! they may drift between releases.

use std::marker::PhantomData;

use telers::types::{InlineKeyboardButton, Update};
use telers::{FilterResult, Request};

use crate::callback_data::CallbackDataTrait;

/// Fixed context key under which the filter stores `Option<i64>` — the
/// matched callback data's `owner_id()`, if any.
pub const OWNER_ID_CONTEXT_KEY: &str = "callback_data_owner_id";

/// A zero-sized filter for `T` — matches an update whose callback query's
/// `data` successfully unpacks into `T`, inserts the unpacked value into
/// the request context under the key `"callback_data"`, and always writes
/// `owner_id()` (an `Option<i64>`) under [`OWNER_ID_CONTEXT_KEY`].
#[derive(Debug, Clone, Copy)]
pub struct CallbackDataFilter<T>(PhantomData<T>);

impl<T> Default for CallbackDataFilter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> CallbackDataFilter<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

/// Try to extract & unpack `T` from an update's callback query. Returns
/// `None` (not an error) for "this update doesn't match" — mismatches are
/// expected and routine (wrong prefix, non-callback update, stale button),
/// not something worth failing a filter chain over.
pub async fn try_unpack_from_update<T>(update: &Update) -> Option<T>
where
    T: CallbackDataTrait,
{
    let call = update.callback_query()?;
    let raw = call.data.as_ref()?;
    T::unpack(raw).ok()
}

impl<T, Client> telers::Filter<Client> for CallbackDataFilter<T>
where
    T: CallbackDataTrait + Clone + Send + Sync + 'static,
    Client: Send + Sync + 'static,
{
    type Error = std::convert::Infallible;

    async fn check(&mut self, request: &mut Request<Client>) -> FilterResult<Self::Error> {
        let Some(data) = try_unpack_from_update::<T>(&request.update).await else {
            return Ok(false);
        };
        let owner_id = data.owner_id();
        request.context.insert("callback_data", data);
        // Always overwritten on every match — never left over from a
        // previous filter in the chain, even when this T has no owner field.
        request.context.insert(OWNER_ID_CONTEXT_KEY, owner_id);
        Ok(true)
    }
}

/// Convenience extension for building an inline button directly from a
/// callback-data value.
pub trait CallbackDataButtonExt: CallbackDataTrait {
    fn button(
        &self,
        text: impl Into<String>,
    ) -> Result<InlineKeyboardButton, crate::error::PackError> {
        Ok(InlineKeyboardButton::new(text.into()).callback_data(self.pack()?))
    }
}

impl<T: CallbackDataTrait> CallbackDataButtonExt for T {}

/// Type alias kept around for readability at call sites: `CallbackDataFilter<T>::new()`.
pub fn as_filter<T: CallbackDataTrait>() -> CallbackDataFilter<T> {
    CallbackDataFilter::new()
}
