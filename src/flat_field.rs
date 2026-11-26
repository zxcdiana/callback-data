use crate::error::{Mapper, NONE_MARKER, PackError, SEP, SOME_MARKER, UnpackError};
use std::any::type_name;
use std::str::FromStr;
use std::vec::IntoIter;

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

pub(crate) fn next_token(
    tokens: &mut IntoIter<String>,
    field: &'static str,
) -> Result<String, UnpackError> {
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
    f32 => "f32", f64 => "f64",
}

macro_rules! abs {
    (i8, $v:expr) => {
        ($v).abs()
    };
    (i16, $v:expr) => {
        ($v).abs()
    };
    (i32, $v:expr) => {
        ($v).abs()
    };
    (i64, $v:expr) => {
        ($v).abs()
    };
    (i128, $v:expr) => {
        ($v).abs()
    };
    (isize, $v:expr) => {
        ($v).abs()
    };

    ($ty:ty, $v:expr) => {
        $v
    };
}

macro_rules! neg {
    (i8, $v:expr) => {
        Result::<i8, UnpackError>::Ok(-($v))
    };
    (i16, $v:expr) => {
        Result::<i16, UnpackError>::Ok(-($v))
    };
    (i32, $v:expr) => {
        Result::<i32, UnpackError>::Ok(-($v))
    };
    (i64, $v:expr) => {
        Result::<i64, UnpackError>::Ok(-($v))
    };
    (i128, $v:expr) => {
        Result::<i128, UnpackError>::Ok(-($v))
    };
    (isize, $v:expr) => {
        Result::<isize, UnpackError>::Ok(-($v))
    };

    ($ty:ty, $v:expr) => {
        Result::<$ty, UnpackError>::Err(
            Mapper::new(&($v).to_string(), type_name::<$ty>())
                .err("cannot convert {$ty} `{$v}` to negative"),
        )
    };
}

macro_rules! impl_integer {
    ($ty:ty) => {
        impl FlatField for $ty {
            const TOKEN_COUNT: usize = 1;

            fn pack_into(&self, out: &mut Vec<String>) -> Result<(), PackError> {
                let mut num = *self;
                #[allow(unused_comparisons)]
                let negative = num < 0;

                if negative {
                    #[allow(clippy::self_assignment)]
                    {
                        num = abs!(Self, num);
                    }
                }

                let part = u128::try_from(num)
                    .ok()
                    .map(base62::encode)
                    .map(|mut encoded| {
                        if negative {
                            unsafe {
                                encoded.as_mut_vec().insert(0, b'-');
                            }
                        }
                        encoded
                    })
                    .unwrap_or_else(|| self.to_string());

                out.push(part);

                Ok(())
            }

            fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, UnpackError> {
                let t = next_token(tokens, type_name::<Self>())?;

                Self::from_str(&t).or_else(|_| {
                    let m = Mapper::new(&t, type_name::<Self>());
                    let negative = t.starts_with('-');
                    let num = t.strip_prefix('-').unwrap_or(&t);

                    base62::decode(num)
                        .map_err(|e| m.err(e))
                        .and_then(|decoded: u128| Self::try_from(decoded).map_err(|e| m.err(e)))
                        .and_then(|converted: Self| {
                            if negative {
                                neg!(Self, converted)
                            } else {
                                Ok(converted)
                            }
                        })
                })
            }
        }
    };
}

impl_integer!(i8);
impl_integer!(i16);
impl_integer!(i32);
impl_integer!(i64);
impl_integer!(i128);
impl_integer!(isize);
impl_integer!(u8);
impl_integer!(u16);
impl_integer!(u32);
impl_integer!(u64);
impl_integer!(u128);
impl_integer!(usize);

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
