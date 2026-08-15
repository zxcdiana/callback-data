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
}
