//! Input parameter representation.

use super::{BindParam, IntoBindParamIn, Value, bytes_of, indicator};
use crate::ffi::YacExtType;
use crate::types::{Date, IntervalDS, IntervalYM, Number, Time, Timestamp, YacNumber, YacTimestamp};
use std::borrow::Cow;

pub(super) enum Input<'a> {
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
    Text(Option<Cow<'a, str>>),
    Binary(Option<Cow<'a, [u8]>>),
}

impl Input<'_> {
    pub(super) fn native_value(&mut self) -> (YacExtType, &[u8]) {
        match self {
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
        }
    }
}

#[inline]
fn scalar_value<T>(value: &mut Option<T>) -> &[u8] {
    match value.as_mut() {
        Some(value) => bytes_of(value),
        None => &[],
    }
}

impl<'a> IntoBindParamIn<'a> for bool {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Bool(Some(self))),
            indicator: indicator(size_of::<bool>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<bool> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Bool(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<bool>())),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for i8 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::I8(Some(self))),
            indicator: indicator(size_of::<i8>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<i8> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i8>())),
            value: Value::Input(Input::I8(self)),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for i16 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::I16(Some(self))),
            indicator: indicator(size_of::<i16>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<i16> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i16>())),
            value: Value::Input(Input::I16(self)),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for i32 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::I32(Some(self))),
            indicator: indicator(size_of::<i32>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<i32> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i32>())),
            value: Value::Input(Input::I32(self)),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for i64 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::I64(Some(self))),
            indicator: indicator(size_of::<i64>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<i64> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<i64>())),
            value: Value::Input(Input::I64(self)),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for f32 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::F32(Some(self))),
            indicator: indicator(size_of::<f32>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<f32> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<f32>())),
            value: Value::Input(Input::F32(self)),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for f64 {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::F64(Some(self))),
            indicator: indicator(size_of::<f64>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<f64> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<f64>())),
            value: Value::Input(Input::F64(self)),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Number {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Number(YacNumber::from_number(self))),
            indicator: indicator(size_of::<YacNumber>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<Number> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
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

impl<'a> IntoBindParamIn<'a> for Date {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Date(Some(self))),
            indicator: indicator(size_of::<Date>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<Date> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Date(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<Date>())),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Time {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Time(Some(self))),
            indicator: indicator(size_of::<Time>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<Time> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Time(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<Time>())),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Timestamp {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Timestamp(YacTimestamp::with_timestamp(self))),
            indicator: indicator(size_of::<YacTimestamp>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<Timestamp> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
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

impl<'a> IntoBindParamIn<'a> for IntervalYM {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::IntervalYM(Some(self))),
            indicator: indicator(size_of::<IntervalYM>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<IntervalYM> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::IntervalYM(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<IntervalYM>())),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for IntervalDS {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::IntervalDS(Some(self))),
            indicator: indicator(size_of::<IntervalDS>()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<IntervalDS> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::IntervalDS(self)),
            indicator: self
                .as_ref()
                .map_or(crate::ffi::NULL_DATA, |_| indicator(size_of::<IntervalDS>())),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for &'a str {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Text(Some(Cow::Borrowed(self)))),
            indicator: indicator(self.len()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<&'a str> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        let indicator = self.map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Text(self.map(Cow::Borrowed))),
            indicator,
        }
    }
}

impl<'a> IntoBindParamIn<'a> for String {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        let indicator = indicator(self.len());
        BindParam {
            value: Value::Input(Input::Text(Some(Cow::Owned(self)))),
            indicator,
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<String> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Text(self.map(Cow::Owned))),
            indicator,
        }
    }
}

impl<'a> IntoBindParamIn<'a> for &'a [u8] {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        BindParam {
            value: Value::Input(Input::Binary(Some(Cow::Borrowed(self)))),
            indicator: indicator(self.len()),
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<&'a [u8]> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        let indicator = self.map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Binary(self.map(Cow::Borrowed))),
            indicator,
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Vec<u8> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        let indicator = indicator(self.len());
        BindParam {
            value: Value::Input(Input::Binary(Some(Cow::Owned(self)))),
            indicator,
        }
    }
}

impl<'a> IntoBindParamIn<'a> for Option<Vec<u8>> {
    #[inline]
    fn into_bind_param_in(self) -> BindParam<'a> {
        let indicator = self
            .as_ref()
            .map_or(crate::ffi::NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::Input(Input::Binary(self.map(Cow::Owned))),
            indicator,
        }
    }
}
