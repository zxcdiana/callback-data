use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use std::vec::IntoIter;

use crate::ChronoError;
use crate::FlatField;

impl FlatField for NaiveTime {
    const TOKEN_COUNT: usize = 1;

    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), crate::PackError> {
        self.num_seconds_from_midnight().pack_into(out)
    }

    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, crate::UnpackError> {
        u32::unpack_from(tokens).and_then(|secs| {
            Self::from_num_seconds_from_midnight_opt(secs, 0)
                .ok_or_else(|| ChronoError::InvalidTime(secs).into())
        })
    }
}

impl FlatField for NaiveDate {
    const TOKEN_COUNT: usize = 1;

    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), crate::PackError> {
        self.num_days_from_ce().pack_into(out)
    }

    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, crate::UnpackError> {
        i32::unpack_from(tokens).and_then(|days| {
            Self::from_num_days_from_ce_opt(days)
                .ok_or_else(|| ChronoError::InvalidDate(days).into())
        })
    }
}
impl FlatField for NaiveDateTime {
    const TOKEN_COUNT: usize = 1;

    fn pack_into(&self, out: &mut Vec<String>) -> Result<(), crate::PackError> {
        self.and_utc().timestamp().pack_into(out)
    }

    fn unpack_from(tokens: &mut IntoIter<String>) -> Result<Self, crate::UnpackError> {
        let ts = i64::unpack_from(tokens)?;
        DateTime::from_timestamp(ts, 0)
            .map(|x| x.naive_utc())
            .ok_or_else(|| crate::ChronoError::InvalidTimestamp(ts).into())
    }
}
