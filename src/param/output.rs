//! Output parameter representation.

use super::{BindParam, IntoBindParamInOut, IntoBindParamOut, Value, bytes_of, indicator};
use crate::conn::Connection;
use crate::error::Error;
use crate::ffi::{NULL_DATA, YacExtType};
use crate::types::{Date, IntervalDS, IntervalYM, Number, Time, Timestamp, YacNumber, YacTimestamp, YasonBuf};
use crate::{Blob, Clob};

pub(super) enum Output<'conn, 'param> {
    Bool(&'param mut bool),
    BoolNullable(&'param mut Option<bool>, bool),
    I8(&'param mut i8),
    I8Nullable(&'param mut Option<i8>, i8),
    I16(&'param mut i16),
    I16Nullable(&'param mut Option<i16>, i16),
    I32(&'param mut i32),
    I32Nullable(&'param mut Option<i32>, i32),
    I64(&'param mut i64),
    I64Nullable(&'param mut Option<i64>, i64),
    F32(&'param mut f32),
    F32Nullable(&'param mut Option<f32>, f32),
    F64(&'param mut f64),
    F64Nullable(&'param mut Option<f64>, f64),
    Number(&'param mut Number, YacNumber),
    NumberNullable(&'param mut Option<Number>, YacNumber),
    Date(&'param mut Date),
    DateNullable(&'param mut Option<Date>, Date),
    Time(&'param mut Time),
    TimeNullable(&'param mut Option<Time>, Time),
    Timestamp(&'param mut Timestamp, YacTimestamp),
    TimestampNullable(&'param mut Option<Timestamp>, YacTimestamp),
    IntervalYM(&'param mut IntervalYM),
    IntervalYMNullable(&'param mut Option<IntervalYM>, IntervalYM),
    IntervalDS(&'param mut IntervalDS),
    IntervalDSNullable(&'param mut Option<IntervalDS>, IntervalDS),
    Text(&'param mut String),
    TextNullable(&'param mut Option<String>, Option<Vec<u8>>, usize),
    Binary(&'param mut Vec<u8>),
    BinaryNullable(&'param mut Option<Vec<u8>>, Option<Vec<u8>>, usize),
    Json(&'param mut YasonBuf, Option<Blob<'conn>>),
    JsonNullable(&'param mut Option<YasonBuf>, Option<Blob<'conn>>),
    Blob(&'param mut Blob<'conn>),
    BlobNullable(&'param mut Option<Blob<'conn>>, Option<Blob<'conn>>),
    Clob(&'param mut Clob<'conn>),
    ClobNullable(&'param mut Option<Clob<'conn>>, Option<Clob<'conn>>),
}

impl<'conn, 'param> Output<'conn, 'param> {
    pub(super) fn native_value(&mut self, conn: &'conn Connection) -> Result<(YacExtType, &mut [u8]), Error> {
        let value = match self {
            Output::Bool(v) => (YacExtType::Bool, bytes_of(*v)),
            Output::BoolNullable(_, v) => (YacExtType::Bool, bytes_of(v)),
            Output::I8(v) => (YacExtType::TinyInt, bytes_of(*v)),
            Output::I8Nullable(_, v) => (YacExtType::TinyInt, bytes_of(v)),
            Output::I16(v) => (YacExtType::SmallInt, bytes_of(*v)),
            Output::I16Nullable(_, v) => (YacExtType::SmallInt, bytes_of(v)),
            Output::I32(v) => (YacExtType::Integer, bytes_of(*v)),
            Output::I32Nullable(_, v) => (YacExtType::Integer, bytes_of(v)),
            Output::I64(v) => (YacExtType::BigInt, bytes_of(*v)),
            Output::I64Nullable(_, v) => (YacExtType::BigInt, bytes_of(v)),
            Output::F32(v) => (YacExtType::Float, bytes_of(*v)),
            Output::F32Nullable(_, v) => (YacExtType::Float, bytes_of(v)),
            Output::F64(v) => (YacExtType::Double, bytes_of(*v)),
            Output::F64Nullable(_, v) => (YacExtType::Double, bytes_of(v)),
            Output::Number(_, v) => (YacExtType::Number, bytes_of(v)),
            Output::NumberNullable(_, v) => (YacExtType::Number, bytes_of(v)),
            Output::Date(v) => (YacExtType::Date, bytes_of(*v)),
            Output::DateNullable(_, v) => (YacExtType::Date, bytes_of(v)),
            Output::Time(v) => (YacExtType::ShortTime, bytes_of(*v)),
            Output::TimeNullable(_, v) => (YacExtType::ShortTime, bytes_of(v)),
            Output::Timestamp(_, v) => (YacExtType::Timestamp, bytes_of(v)),
            Output::TimestampNullable(_, v) => (YacExtType::Timestamp, bytes_of(v)),
            Output::IntervalYM(v) => (YacExtType::YmInterval, bytes_of(*v)),
            Output::IntervalYMNullable(_, v) => (YacExtType::YmInterval, bytes_of(v)),
            Output::IntervalDS(v) => (YacExtType::DsInterval, bytes_of(*v)),
            Output::IntervalDSNullable(_, v) => (YacExtType::DsInterval, bytes_of(v)),
            Output::Text(v) => (YacExtType::Varchar2, unsafe {
                std::slice::from_raw_parts_mut(v.as_mut_ptr(), v.capacity())
            }),
            Output::Binary(v) => (YacExtType::Binary2, unsafe {
                std::slice::from_raw_parts_mut(v.as_mut_ptr(), v.capacity())
            }),
            Output::TextNullable(target, native, capacity) => {
                let native = native.get_or_insert_with(|| nullable_text_buffer(target, *capacity));
                (YacExtType::Varchar2, unsafe {
                    std::slice::from_raw_parts_mut(native.as_mut_ptr(), native.capacity())
                })
            }
            Output::BinaryNullable(target, native, capacity) => {
                let native = native.get_or_insert_with(|| nullable_binary_buffer(target, *capacity));
                (YacExtType::Binary2, unsafe {
                    std::slice::from_raw_parts_mut(native.as_mut_ptr(), native.capacity())
                })
            }
            Output::Json(_, value) | Output::JsonNullable(_, value) => {
                let value = value.get_or_insert(Blob::output(conn)?);
                (YacExtType::Json, value.bind_output())
            }
            // The mutable bytes are the locator-pointer variable itself. YACLI
            // receives its address (`YacLobLocator **`) and updates that
            // variable according to the output binding protocol.
            Output::Blob(value) => (YacExtType::Blob, value.bind_output()),
            Output::BlobNullable(_, value) => {
                let value = value.get_or_insert(Blob::output(conn)?);
                (YacExtType::Blob, value.bind_output())
            }
            Output::Clob(value) => (YacExtType::Clob, value.bind_output()),
            Output::ClobNullable(_, value) => {
                let value = value.get_or_insert(Clob::output(conn)?);
                (YacExtType::Clob, value.bind_output())
            }
        };
        Ok(value)
    }

    pub(super) fn complete(&mut self, indicator: i32) -> Result<(), Error> {
        if indicator < NULL_DATA {
            return Err(Error::Internal(
                "client returned an invalid output parameter indicator".into(),
            ));
        }

        match self {
            Output::BoolNullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::I8Nullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::I16Nullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::I32Nullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::I64Nullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::F32Nullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::F64Nullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::NumberNullable(t, n) => {
                **t = (indicator != NULL_DATA).then(|| n.into_number());
            }
            Output::DateNullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::TimeNullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::TimestampNullable(t, n) => {
                **t = (indicator != NULL_DATA).then_some(n.timestamp);
            }
            Output::IntervalYMNullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::IntervalDSNullable(t, n) => **t = (indicator != NULL_DATA).then_some(*n),
            Output::Number(t, n) => **t = n.into_number(),
            Output::Timestamp(t, n) => **t = n.timestamp,
            Output::Text(t) => {
                let len = output_len(indicator, t.capacity(), "text")?;
                // The client returns UTF-8 because the connection charset is set to UTF-8.
                // SAFETY: YACLI guarantees that text output uses the UTF-8 connection charset.
                unsafe { t.as_mut_vec().set_len(len) };
            }
            Output::Binary(t) => {
                let len = output_len(indicator, t.capacity(), "binary")?;
                unsafe { t.set_len(len) };
            }
            Output::TextNullable(t, n, _) => {
                if indicator == NULL_DATA {
                    **t = None;
                } else {
                    let n = n.as_mut().expect("text output buffer was not initialized");
                    let len = output_len(indicator, n.capacity(), "text")?;
                    // The client returns UTF-8 because the connection charset is set to UTF-8.
                    // SAFETY: YACLI guarantees that text output uses the UTF-8 connection charset.
                    unsafe { n.set_len(len) };
                    // SAFETY: YACLI guarantees that text output uses the UTF-8 connection charset.
                    **t = Some(unsafe { String::from_utf8_unchecked(std::mem::take(n)) });
                }
            }
            Output::BinaryNullable(t, n, _) => {
                if indicator == NULL_DATA {
                    **t = None;
                } else {
                    let n = n.as_mut().expect("binary output buffer was not initialized");
                    let len = output_len(indicator, n.capacity(), "binary")?;
                    unsafe { n.set_len(len) };
                    **t = Some(std::mem::take(n));
                }
            }
            Output::Json(target, value) => {
                if indicator == NULL_DATA {
                    return Err(Error::InvalidArgument(
                        "SQL NULL cannot be written to a non-optional JSON output parameter".into(),
                    ));
                }
                let mut bytes = Vec::new();
                value
                    .as_mut()
                    .expect("JSON output locator was not initialized")
                    .read_to_end(&mut bytes)?;
                // JSON values returned by the database are encoded as valid YASON.
                **target = unsafe { YasonBuf::new_unchecked(bytes) };
                *value = None;
            }
            Output::JsonNullable(target, value) => {
                if indicator == NULL_DATA {
                    **target = None;
                } else {
                    let mut bytes = Vec::new();
                    value
                        .as_mut()
                        .expect("JSON output locator was not initialized")
                        .read_to_end(&mut bytes)?;
                    // JSON values returned by the database are encoded as valid YASON.
                    **target = Some(unsafe { YasonBuf::new_unchecked(bytes) });
                }
                *value = None;
            }
            Output::Blob(_) | Output::Clob(_) => {
                if indicator == NULL_DATA {
                    return Err(Error::InvalidArgument(
                        "SQL NULL cannot be written to a non-optional output LOB".into(),
                    ));
                }
            }
            Output::BlobNullable(target, value) => {
                **target = if indicator == NULL_DATA { None } else { value.take() };
            }
            Output::ClobNullable(target, value) => {
                **target = if indicator == NULL_DATA { None } else { value.take() };
            }
            _ => {
                if indicator == NULL_DATA {
                    return Err(Error::InvalidArgument(
                        "SQL NULL cannot be written to a non-optional output parameter".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[inline]
fn output_len(indicator: i32, capacity: usize, kind: &str) -> Result<usize, Error> {
    let len = indicator as usize;
    if len > capacity {
        return Err(Error::InvalidArgument(format!("{kind} output exceeds its capacity")));
    }
    Ok(len)
}

#[inline]
fn nullable_text_buffer(target: &mut Option<String>, capacity: usize) -> Vec<u8> {
    let mut native = match target.take() {
        Some(value) => value.into_bytes(),
        None => Vec::new(),
    };
    native.reserve(capacity.saturating_sub(native.len()));
    native
}

#[inline]
fn nullable_binary_buffer(target: &mut Option<Vec<u8>>, capacity: usize) -> Vec<u8> {
    let mut native = target.take().unwrap_or_default();
    native.reserve(capacity.saturating_sub(native.len()));
    native
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut YasonBuf {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Json(self, None)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<YasonBuf> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::JsonNullable(self, None)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut bool {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Bool(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut bool {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::Bool(self)),
            indicator: indicator(size_of::<bool>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<bool> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::BoolNullable(self, false)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<bool> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, false), |value| (indicator(size_of::<bool>()), *value));
        BindParam {
            value: Value::InOut(Output::BoolNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut i8 {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I8(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut i8 {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::I8(self)),
            indicator: indicator(size_of::<i8>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<i8> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I8Nullable(self, 0)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<i8> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, 0), |value| (indicator(size_of::<i8>()), *value));
        BindParam {
            value: Value::InOut(Output::I8Nullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut i16 {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I16(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut i16 {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::I16(self)),
            indicator: indicator(size_of::<i16>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<i16> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I16Nullable(self, 0)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<i16> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, 0), |value| (indicator(size_of::<i16>()), *value));
        BindParam {
            value: Value::InOut(Output::I16Nullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut i32 {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I32(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut i32 {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::I32(self)),
            indicator: indicator(size_of::<i32>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<i32> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I32Nullable(self, 0)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<i32> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, 0), |value| (indicator(size_of::<i32>()), *value));
        BindParam {
            value: Value::InOut(Output::I32Nullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut i64 {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I64(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut i64 {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::I64(self)),
            indicator: indicator(size_of::<i64>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<i64> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::I64Nullable(self, 0)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<i64> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, 0), |value| (indicator(size_of::<i64>()), *value));
        BindParam {
            value: Value::InOut(Output::I64Nullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut f32 {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::F32(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut f32 {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::F32(self)),
            indicator: indicator(size_of::<f32>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<f32> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::F32Nullable(self, 0.0)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<f32> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, 0.0), |value| (indicator(size_of::<f32>()), *value));
        BindParam {
            value: Value::InOut(Output::F32Nullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut f64 {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::F64(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut f64 {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::F64(self)),
            indicator: indicator(size_of::<f64>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<f64> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::F64Nullable(self, 0.0)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<f64> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, 0.0), |value| (indicator(size_of::<f64>()), *value));
        BindParam {
            value: Value::InOut(Output::F64Nullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Date {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Date(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Date {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::Date(self)),
            indicator: indicator(size_of::<Date>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<Date> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::DateNullable(self, Date::MIN)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<Date> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, Date::MIN), |value| (indicator(size_of::<Date>()), *value));
        BindParam {
            value: Value::InOut(Output::DateNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Time {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Time(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Time {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::Time(self)),
            indicator: indicator(size_of::<Time>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<Time> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::TimeNullable(self, Time::ZERO)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<Time> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self
            .as_ref()
            .map_or((NULL_DATA, Time::ZERO), |value| (indicator(size_of::<Time>()), *value));
        BindParam {
            value: Value::InOut(Output::TimeNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut IntervalYM {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::IntervalYM(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut IntervalYM {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::IntervalYM(self)),
            indicator: indicator(size_of::<IntervalYM>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<IntervalYM> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::IntervalYMNullable(self, IntervalYM::ZERO)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<IntervalYM> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self.as_ref().map_or((NULL_DATA, IntervalYM::ZERO), |value| {
            (indicator(size_of::<IntervalYM>()), *value)
        });
        BindParam {
            value: Value::InOut(Output::IntervalYMNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut IntervalDS {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::IntervalDS(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut IntervalDS {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::InOut(Output::IntervalDS(self)),
            indicator: indicator(size_of::<IntervalDS>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<IntervalDS> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::IntervalDSNullable(self, IntervalDS::ZERO)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<IntervalDS> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self.as_ref().map_or((NULL_DATA, IntervalDS::ZERO), |value| {
            (indicator(size_of::<IntervalDS>()), *value)
        });
        BindParam {
            value: Value::InOut(Output::IntervalDSNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Number {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Number(self, YacNumber::ZERO)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Number {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let native = YacNumber::from_number(*self);
        BindParam {
            value: Value::InOut(Output::Number(self, native)),
            indicator: indicator(size_of::<YacNumber>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<Number> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::NumberNullable(self, YacNumber::ZERO)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<Number> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) = self.as_ref().map_or((NULL_DATA, YacNumber::ZERO), |value| {
            (indicator(size_of::<YacNumber>()), YacNumber::from_number(*value))
        });
        BindParam {
            value: Value::InOut(Output::NumberNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Timestamp {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Timestamp(self, YacTimestamp::with_timestamp(Timestamp::MIN))),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Timestamp {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let native = YacTimestamp::with_timestamp(*self);
        BindParam {
            value: Value::InOut(Output::Timestamp(self, native)),
            indicator: indicator(size_of::<YacTimestamp>()),
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<Timestamp> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::TimestampNullable(
                self,
                YacTimestamp::with_timestamp(Timestamp::MIN),
            )),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Option<Timestamp> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let (indicator, native) =
            self.as_ref()
                .map_or((NULL_DATA, YacTimestamp::with_timestamp(Timestamp::MIN)), |value| {
                    (
                        indicator(size_of::<YacTimestamp>()),
                        YacTimestamp::with_timestamp(*value),
                    )
                });
        BindParam {
            value: Value::InOut(Output::TimestampNullable(self, native)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut String {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Text(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut String {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let indicator = indicator(self.len());
        BindParam {
            value: Value::InOut(Output::Text(self)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for (&'param mut Option<String>, usize) {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::TextNullable(self.0, None, self.1)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for (&'param mut Option<String>, usize) {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let indicator = self.0.as_ref().map_or(NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::InOut(Output::TextNullable(self.0, None, self.1)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Vec<u8> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Binary(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for &'param mut Vec<u8> {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let indicator = indicator(self.len());
        BindParam {
            value: Value::InOut(Output::Binary(self)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for (&'param mut Option<Vec<u8>>, usize) {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::BinaryNullable(self.0, None, self.1)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamInOut<'conn, 'param> for (&'param mut Option<Vec<u8>>, usize) {
    #[inline]
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param> {
        let indicator = self.0.as_ref().map_or(NULL_DATA, |value| indicator(value.len()));
        BindParam {
            value: Value::InOut(Output::BinaryNullable(self.0, None, self.1)),
            indicator,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Blob<'conn> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Blob(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Clob<'conn> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::Clob(self)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<Blob<'conn>> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::BlobNullable(self, None)),
            indicator: 0,
        }
    }
}

impl<'conn, 'param> IntoBindParamOut<'conn, 'param> for &'param mut Option<Clob<'conn>> {
    #[inline]
    fn into_bind_param_out(self) -> BindParam<'conn, 'param> {
        BindParam {
            value: Value::Output(Output::ClobNullable(self, None)),
            indicator: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_text_target_is_unchanged_before_native_binding() {
        let mut target = Some(String::from("keep"));
        let _ = (&mut target, 16).into_bind_param_out();
        assert_eq!(target.as_deref(), Some("keep"));
    }

    #[test]
    fn nullable_binary_target_is_unchanged_before_native_binding() {
        let mut target = Some(vec![1, 2, 3]);
        let _ = (&mut target, 16).into_bind_param_out();
        assert_eq!(target, Some(vec![1, 2, 3]));
    }
}
