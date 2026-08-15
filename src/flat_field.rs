use std::str::FromStr;
use std::vec::IntoIter;

use crate::error::{NONE_MARKER, PackError, SEP, SOME_MARKER, UnpackError};

/// Internal trait implemented for every type that can be packed into a fixed
/// number of `:`-separated tokens.
///
/// This is **not** meant to be implemented by hand outside this crate for
/// arbitrary types (that's exactly the footgun we're avoiding — see the
/// design discussion around untagged enums / `serde_json::Value`). Use
/// `#[derive(FlatField)]` on structs/enums that are meant to be nested
/// inside a `#[callback_data(...)]` type instead.
///
/// `TOKEN_COUNT` must be a value known at compile time and identical for
/// every value of `Self` — this is what lets a parent type know exactly how
/// many tokens to reserve for this field without look-ahead parsing.
pub trait FlatField: Sized {
    /// How many `:`-separated tokens this type always occupies.
    const TOKEN_COUNT: usize;

    /// Push this value's tokens onto `out`, validating that no token
    /// collides with reserved characters.
    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), PackError>;

    /// Consume exactly `Self::TOKEN_COUNT` tokens from `tokens` and build `Self`.
    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, UnpackError>;
}

fn validate_token(value: String) -> Result<String, PackError> {
    if value.contains(SEP) {
        return Err(PackError::ContainsSeparator { value, sep: SEP });
    }
    if value == NONE_MARKER || value == SOME_MARKER {
        return Err(PackError::CollidesWithMarker {
            value,
            marker: NONE_MARKER,
        });
    }
    Ok(value)
}

fn next_token(tokens: &mut IntoIter<String>, field: &'static str) -> Result<String, UnpackError> {
    tokens.next().ok_or(UnpackError::NotEnoughTokens { field })
}

fn parse_display<T>(token: String, ty: &'static str) -> Result<T, UnpackError>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let original = token.clone();
    token.parse::<T>().map_err(|e| UnpackError::ParseFailed {
        token: original,
        ty,
        source: Box::new(e),
    })
}

macro_rules! impl_flat_field_display {
    ($($ty:ty => $name:literal),* $(,)?) => {
        $(
            impl FlatField for $ty {
                const TOKEN_COUNT: usize = 1;

                fn pack_into(&self, out: &mut Vec<String>) -> Result<(), PackError> {
                    out.push(validate_token(self.to_string())?);
                    Ok(())
                }

                fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, UnpackError> {
                    let token = next_token(tokens, $name)?;
                    parse_display(token, $name)
                }
            }
        )*
    };
}

impl_flat_field_display! {
    u8 => "u8", u16 => "u16", u32 => "u32", u64 => "u64", u128 => "u128", usize => "usize",
    i8 => "i8", i16 => "i16", i32 => "i32", i64 => "i64", i128 => "i128", isize => "isize",
    f32 => "f32", f64 => "f64",
}

impl FlatField for bool {
    const TOKEN_COUNT: usize = 1;

    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), PackError> {
        out.push(if *self {
            "t".to_string()
        } else {
            "f".to_string()
        });
        Ok(())
    }

    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, UnpackError> {
        let token = next_token(tokens, "bool")?;
        match token.as_str() {
            "t" => Ok(true),
            "f" => Ok(false),
            _ => Err(UnpackError::ParseFailed {
                token,
                ty: "bool",
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "expected `t` or `f`",
                )),
            }),
        }
    }
}

impl FlatField for String {
    const TOKEN_COUNT: usize = 1;

    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), PackError> {
        out.push(validate_token(self.clone())?);
        Ok(())
    }

    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, UnpackError> {
        next_token(tokens, "String")
    }
}

/// `Option<T>` is encoded as a 1-token tag (`~` = None, `s` = Some) followed
/// by exactly `T::TOKEN_COUNT` tokens, which are ignored (but must still be
/// present as empty placeholders) when the tag is `None`. This keeps the
/// total token count for the field fixed at compile time, regardless of
/// which variant is actually present.
impl<T: FlatField> FlatField for Option<T> {
    const TOKEN_COUNT: usize = 1 + T::TOKEN_COUNT;

    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), PackError> {
        match self {
            None => {
                out.push(NONE_MARKER.to_string());
                for _ in 0..T::TOKEN_COUNT {
                    out.push(String::new());
                }
            }
            Some(value) => {
                out.push(SOME_MARKER.to_string());
                value.pack_into(out)?;
            }
        }
        Ok(())
    }

    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, UnpackError> {
        let tag = next_token(tokens, "Option<T>")?;
        match tag.as_str() {
            NONE_MARKER => {
                for _ in 0..T::TOKEN_COUNT {
                    next_token(tokens, "Option<T>::None padding")?;
                }
                Ok(None)
            }
            SOME_MARKER => Ok(Some(T::unpack_from(tokens)?)),
            _ => Err(UnpackError::InvalidOptionMarker { token: tag }),
        }
    }
}
