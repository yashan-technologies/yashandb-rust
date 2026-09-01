//! Conversion of fetched column values into Rust values.

use super::*;
use crate::lob::{Blob, Clob};

pub trait FromColumn<'conn, 'row>: Sized {
    fn from_column(column: &Column<'conn, 'row>) -> Result<Self, Error>;
}

macro_rules! impl_from_column {
    ($t:ty, $name:literal, $kind:pat, $read:ident) => {
        impl<'conn, 'row> FromColumn<'conn, 'row> for $t {
            #[inline]
            fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
                c.read($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
            }
        }
        impl<'conn, 'row> FromColumn<'conn, 'row> for Option<$t> {
            #[inline]
            fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
                c.read_optional($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
            }
        }
    };
    (borrow $t:ty, $name:literal, $kind:pat, $read:ident) => {
        impl<'conn, 'row> FromColumn<'conn, 'row> for &'row $t {
            #[inline]
            fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
                c.read($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
            }
        }
        impl<'conn, 'row> FromColumn<'conn, 'row> for Option<&'row $t> {
            #[inline]
            fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
                c.read_optional($name, |actual| matches!(actual, $kind), |buffer| buffer.$read())
            }
        }
    };
    (fallible $t:ty, $name:literal, $kind:pat, $read:ident) => {
        impl<'conn, 'row> FromColumn<'conn, 'row> for $t {
            #[inline]
            fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
                c.read(
                    $name,
                    |actual| matches!(actual, $kind),
                    |buffer| buffer.$read(c.index, c.pending_rebinds),
                )?
            }
        }
        impl<'conn, 'row> FromColumn<'conn, 'row> for Option<$t> {
            #[inline]
            fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
                c.read_optional(
                    $name,
                    |actual| matches!(actual, $kind),
                    |buffer| buffer.$read(c.index, c.pending_rebinds),
                )?
                .transpose()
            }
        }
    };
}

impl_from_column!(bool, "bool", DataTypeInfo::Bool, as_bool);

impl_from_column!(i8, "i8", DataTypeInfo::TinyInt, as_i8);
impl_from_column!(i16, "i16", DataTypeInfo::SmallInt, as_i16);
impl_from_column!(i32, "i32", DataTypeInfo::Integer, as_i32);
impl_from_column!(i64, "i64", DataTypeInfo::BigInt, as_i64);
impl_from_column!(f32, "f32", DataTypeInfo::Float, as_f32);
impl_from_column!(f64, "f64", DataTypeInfo::Double, as_f64);
impl_from_column!(Number, "Number", DataTypeInfo::Number { .. }, as_number);

impl_from_column!(Date, "Date", DataTypeInfo::Date, as_date);
impl_from_column!(Time, "Time", DataTypeInfo::Time, as_time);
impl_from_column!(Timestamp, "Timestamp", DataTypeInfo::Timestamp, as_timestamp);
impl_from_column!(IntervalYM, "IntervalYM", DataTypeInfo::IntervalYM, as_interval_ym);
impl_from_column!(IntervalDS, "IntervalDS", DataTypeInfo::IntervalDS, as_interval_ds);

impl_from_column!(
    borrow str,
    "&str",
    DataTypeInfo::Char { .. }
        | DataTypeInfo::VarChar { .. }
        | DataTypeInfo::NChar { .. }
        | DataTypeInfo::NVarChar { .. },
    as_text
);
impl_from_column!(
    String,
    "String",
    DataTypeInfo::Char { .. }
        | DataTypeInfo::VarChar { .. }
        | DataTypeInfo::NChar { .. }
        | DataTypeInfo::NVarChar { .. },
    as_string
);
impl_from_column!(borrow[u8], "&[u8]", DataTypeInfo::Binary { .. }, as_binary);
impl_from_column!(Vec<u8>, "Vec<u8>", DataTypeInfo::Binary { .. }, as_vec);

impl_from_column!(fallible Blob<'conn>, "Blob", DataTypeInfo::Blob, as_blob);

impl<'conn, 'row> FromColumn<'conn, 'row> for Clob<'conn> {
    #[inline]
    fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
        c.read(
            "Clob",
            |actual| matches!(actual, DataTypeInfo::Clob | DataTypeInfo::Nclob),
            |buffer| buffer.as_clob(c.index, c.pending_rebinds, c.info.data_type_info),
        )?
    }
}

impl<'conn, 'row> FromColumn<'conn, 'row> for Option<Clob<'conn>> {
    #[inline]
    fn from_column(c: &Column<'conn, 'row>) -> Result<Self, Error> {
        c.read_optional(
            "Clob",
            |actual| matches!(actual, DataTypeInfo::Clob | DataTypeInfo::Nclob),
            |buffer| buffer.as_clob(c.index, c.pending_rebinds, c.info.data_type_info),
        )?
        .transpose()
    }
}
