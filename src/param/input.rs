//! Input parameter representation.

use super::{BindParam, IntoBindParamIn, Value, bytes_of, indicator};
use crate::ffi::YacExtType;
use crate::types::{Date, IntervalDS, IntervalYM, Number, Time, Timestamp, YacNumber, YacTimestamp, Yason, YasonBuf};
use crate::{Blob, Clob};
use std::borrow::Cow;

pub(super) enum Input<'conn, 'param> {
    Bool(Option<bool>),
    I8(Option<i8>),
    I16(Option<i16>),
    I32(Option<i32>),
    I64(Option<i64>),
    F32(Option<f32>),
    F64(Option<f64>),
    Number(YacNumber),
    Date(Option<Date>),
    Time(Option<Time>),
    Timestamp(YacTimestamp),
    IntervalYM(Option<IntervalYM>),
    IntervalDS(Option<IntervalDS>),
    Text(Option<Cow<'param, str>>),
    Binary(Option<Cow<'param, [u8]>>),
    Json {
        value: Option<Cow<'param, Yason>>,
        temporary_blob: Option<Blob<'conn>>,
    },
    Blob(Option<&'param Blob<'conn>>),
    Clob(Option<&'param Clob<'conn>>),
}

impl<'conn, 'param> Input<'conn, 'param> {
    pub(super) fn native_value(
        &mut self,
        conn: &'conn crate::conn::Connection,
    ) -> Result<(YacExtType, &[u8]), crate::Error> {
        let value: (YacExtType, &[u8]) = match self {
            Input::Bool(v) => (YacExtType::Bool, scalar_value(v)),
            Input::I8(v) => (YacExtType::TinyInt, scalar_value(v)),
            Input::I16(v) => (YacExtType::SmallInt, scalar_value(v)),
            Input::I32(v) => (YacExtType::Integer, scalar_value(v)),
            Input::I64(v) => (YacExtType::BigInt, scalar_value(v)),
            Input::F32(v) => (YacExtType::Float, scalar_value(v)),
            Input::F64(v) => (YacExtType::Double, scalar_value(v)),
            Input::Number(v) => (YacExtType::Number, bytes_of(v)),
            Input::Date(v) => (YacExtType::Date, scalar_value(v)),
            Input::Time(v) => (YacExtType::ShortTime, scalar_value(v)),
            Input::Timestamp(v) => (YacExtType::Timestamp, bytes_of(v)),
            Input::IntervalYM(v) => (YacExtType::YmInterval, scalar_value(v)),
            Input::IntervalDS(v) => (YacExtType::DsInterval, scalar_value(v)),
            Input::Text(v) => (
                YacExtType::Varchar2,
                v.as_deref().map(str::as_bytes).unwrap_or_default(),
            ),
            Input::Binary(v) => (YacExtType::Binary2, v.as_deref().unwrap_or_default()),
            Input::Json { value, temporary_blob } => {
                *temporary_blob = None;
                let Some(value) = value.as_deref() else {
                    return Ok((YacExtType::Json, &[]));
                };
                let mut blob = Blob::temporary(conn)?;
                blob.append(value.as_bytes())?;
                *temporary_blob = Some(blob);
                (
                    YacExtType::Json,
                    temporary_blob.as_ref().expect("temporary JSON BLOB").bind_input(),
                )
            }
            // `bind_input` returns the bytes of the locator-pointer variable;
            // its address is passed to YACLI as `YacLobLocator **`.
            Input::Blob(v) => (YacExtType::Blob, v.map(|v| v.bind_input()).unwrap_or_default()),
            Input::Clob(v) => (YacExtType::Clob, v.map(|v| v.bind_input()).unwrap_or_default()),
        };
        Ok(value)
    }
}

#[inline]
fn scalar_value<T>(value: &mut Option<T>) -> &[u8] {
    match value.as_mut() {
        Some(value) => bytes_of(value),
        None => &[],
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for bool {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Bool(Some(self))),
            indicator: indicator(size_of::<bool>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<bool> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Bool(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<bool>())),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for i8 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::I8(Some(self))),
            indicator: indicator(size_of::<i8>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<i8> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i8>())),
            value: Value::Input(Input::I8(self)),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for i16 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::I16(Some(self))),
            indicator: indicator(size_of::<i16>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<i16> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i16>())),
            value: Value::Input(Input::I16(self)),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for i32 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::I32(Some(self))),
            indicator: indicator(size_of::<i32>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<i32> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i32>())),
            value: Value::Input(Input::I32(self)),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for i64 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::I64(Some(self))),
            indicator: indicator(size_of::<i64>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<i64> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i64>())),
            value: Value::Input(Input::I64(self)),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for f32 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::F32(Some(self))),
            indicator: indicator(size_of::<f32>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<f32> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<f32>())),
            value: Value::Input(Input::F32(self)),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for f64 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::F64(Some(self))),
            indicator: indicator(size_of::<f64>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<f64> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<f64>())),
            value: Value::Input(Input::F64(self)),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Number {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Number(YacNumber::from_number(self))),
            indicator: indicator(size_of::<YacNumber>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<Number> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let native = self.as_ref().copied().map_or(YacNumber::ZERO, YacNumber::from_number);
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<YacNumber>()));

        BindParam {
            value: Value::Input(Input::Number(native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Date {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Date(Some(self))),
            indicator: indicator(size_of::<Date>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<Date> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Date(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<Date>())),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Time {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Time(Some(self))),
            indicator: indicator(size_of::<Time>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<Time> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Time(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<Time>())),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Timestamp {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Timestamp(YacTimestamp::with_timestamp(self))),
            indicator: indicator(size_of::<YacTimestamp>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<Timestamp> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let native = YacTimestamp::with_timestamp(self.as_ref().copied().unwrap_or(Timestamp::MIN));
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<YacTimestamp>()));

        BindParam {
            value: Value::Input(Input::Timestamp(native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for IntervalYM {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::IntervalYM(Some(self))),
            indicator: indicator(size_of::<IntervalYM>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<IntervalYM> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::IntervalYM(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<IntervalYM>())),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for IntervalDS {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::IntervalDS(Some(self))),
            indicator: indicator(size_of::<IntervalDS>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<IntervalDS> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::IntervalDS(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<IntervalDS>())),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for &'param str {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Text(Some(Cow::Borrowed(self)))),
            indicator: indicator(self.len()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<&'param str> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = self.map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Text(self.map(Cow::Borrowed))),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for String {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = indicator(self.len());
        BindParam {
            value: Value::Input(Input::Text(Some(Cow::Owned(self)))),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<String> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Text(self.map(Cow::Owned))),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for &'param [u8] {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Binary(Some(Cow::Borrowed(self)))),
            indicator: indicator(self.len()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<&'param [u8]> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = self.map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Binary(self.map(Cow::Borrowed))),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Vec<u8> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = indicator(self.len());
        BindParam {
            value: Value::Input(Input::Binary(Some(Cow::Owned(self)))),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<Vec<u8>> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Binary(self.map(Cow::Owned))),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for &'param Yason {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Json {
                value: Some(Cow::Borrowed(self)),
                temporary_blob: None,
            }),
            indicator: indicator(self.as_bytes().len()),
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<&'param Yason> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = self.map_or(crate::ffi::NULL_DATA, |value| indicator(value.as_bytes().len()));
        BindParam {
            value: Value::Input(Input::Json {
                value: self.map(Cow::Borrowed),
                temporary_blob: None,
            }),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for &'param YasonBuf {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        self.as_ref().into_bind_param_in()
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<&'param YasonBuf> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        self.map(AsRef::as_ref).into_bind_param_in()
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for YasonBuf {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = indicator(self.as_bytes().len());
        BindParam {
            value: Value::Input(Input::Json {
                value: Some(Cow::Owned(self)),
                temporary_blob: None,
            }),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<YasonBuf> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |value| indicator(value.as_bytes().len()));
        BindParam {
            value: Value::Input(Input::Json {
                value: self.map(Cow::Owned),
                temporary_blob: None,
            }),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for &'param Blob<'conn> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Blob(Some(self))),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for &'param Clob<'conn> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Clob(Some(self))),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<&'param Blob<'conn>> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Blob(self)),
            indicator: if self.is_some() { 0 } else { crate::ffi::NULL_DATA },
        }
    }
}

impl<'conn, 'param> IntoBindParamIn<'conn, 'param> for Option<&'param Clob<'conn>> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Input(Input::Clob(self)),
            indicator: if self.is_some() { 0 } else { crate::ffi::NULL_DATA },
        }
    }
}
