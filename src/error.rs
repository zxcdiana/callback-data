use thiserror::Error;

/// Reserved separator between packed tokens ("field:field:field").
pub const SEP: &str = ":";
/// Reserved marker for `Option::None` in the packed stream.
pub const NONE_MARKER: &str = "~";
/// Reserved marker for `Option::Some` in the packed stream.
pub const SOME_MARKER: &str = "s";

pub const CALLBACK_DATA_MAX_LEN: usize = 64;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("field value `{value}` contains the reserved separator `{sep}`")]
    ContainsSeparator { value: String, sep: &'static str },

    #[error("field value `{value}` collides with the reserved Option marker `{marker}`")]
    CollidesWithMarker { value: String, marker: &'static str },

    #[error("packed callback data `{packed}` has length {len} but the Telegram limit is {max}")]
    TooLong {
        packed: String,
        len: usize,
        max: usize,
    },

    #[error(
        "packed data round-trip check failed: pack() produced data that unpack() rejected: {0}"
    )]
    RoundTripMismatch(#[source] Box<UnpackError>),
}

#[derive(Debug, Error)]
pub enum UnpackError {
    #[error("raw callback data is empty")]
    Empty,

    #[error("prefix mismatch: expected `{expected}`, got `{got}`")]
    PrefixMismatch { expected: String, got: String },

    #[error("token count mismatch: expected {expected} tokens, got {got}")]
    TokenCountMismatch { expected: usize, got: usize },

    #[error("ran out of tokens while decoding field `{field}`")]
    NotEnoughTokens { field: &'static str },

    #[error("failed to parse token `{token}` as `{ty}`: {source}")]
    ParseFailed {
        token: String,
        ty: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("unknown enum variant index `{index}` for `{ty}`")]
    UnknownVariant { index: u32, ty: &'static str },

    #[error("invalid Option marker token `{token}`")]
    InvalidOptionMarker { token: String },

    #[cfg(feature = "chrono")]
    #[error("chrono error: {0:?}")]
    Chrono(ChronoError),
}

#[cfg(feature = "chrono")]
#[derive(Debug, Error)]
pub enum ChronoError {
    #[error("invalid timestamp `{0}`")]
    InvalidTimestamp(i64),
    #[error("invalid num seconds from midnight `{0}`")]
    InvalidTime(u32),
    #[error("invalid num days from ce `{0}`")]
    InvalidDate(i32),
}

#[cfg(feature = "chrono")]
impl<T> From<T> for UnpackError
where
    T: Into<ChronoError>,
{
    fn from(value: T) -> Self {
        value.into().into()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Mapper<'a> {
    pub token: &'a str,
    pub type_name: &'static str,
}
impl<'a> Mapper<'a> {
    pub fn new(token: &'a str, type_name: &'static str) -> Mapper<'a> {
        Self { token, type_name }
    }
    pub fn err(&self, err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> UnpackError {
        UnpackError::ParseFailed {
            token: self.token.into(),
            ty: self.type_name,
            source: err.into(),
        }
    }
}
