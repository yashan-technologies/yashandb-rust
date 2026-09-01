//! Parameter values for prepared statements.

mod input;
mod named;
mod output;

use crate::conn::Connection;
use crate::error::Error;
use crate::ffi::{ParameterBinding, ParameterValue};

use input::Input;
use output::Output;

pub use named::{NamedBindParam, named};

/// A value bound to a prepared statement parameter.
///
/// Values are created with [`input`], [`output`], or [`in_out`]. A parameter
/// list is supplied as an array, slice, or `Vec<BindParam>`. `Option<T>` input
/// values represent SQL `NULL`, while `Option<T>` output targets can receive
/// either a value or SQL `NULL`.
pub struct BindParam<'conn, 'param> {
    value: Value<'conn, 'param>,
    indicator: i32,
}

enum Value<'conn, 'param> {
    Input(Input<'conn, 'param>),
    Output(Output<'conn, 'param>),
    InOut(Output<'conn, 'param>),
}

/// Converts a supported Rust value into an input parameter.
///
/// Implementations are provided for the driver's supported scalar, date/time,
/// interval, text, and binary Rust types, including `Option<T>` forms for SQL
/// `NULL`. This trait is used by [`input`] and is not an extension point.
pub trait IntoBindParamIn<'conn, 'param> {
    /// Convert this value into a parameter.
    fn into_bind_param_in(self) -> BindParam<'conn, 'param>;
}

/// Converts a supported mutable Rust target into an output parameter.
///
/// Fixed-size targets use their native size. `String` and `Vec<u8>` targets
/// use their existing capacity as the output limit. If the client returns more
/// data than the target can hold, execution returns an error. Nullable
/// variable-size targets use the `(target, capacity)` form, for example
/// `output((&mut value, 256))`; `capacity` is the minimum buffer capacity, and
/// an existing larger allocation may be reused. This trait is not an extension
/// point.
pub trait IntoBindParamOut<'conn, 'param> {
    /// Convert this target into a parameter.
    fn into_bind_param_out(self) -> BindParam<'conn, 'param>;
}

/// Converts a supported mutable Rust target into an input/output parameter.
///
/// The target supplies the input value before execution and receives the
/// output value afterward. The same capacity rules as [`IntoBindParamOut`] are
/// used for variable-size targets. This trait is not an extension point.
pub trait IntoBindParamInOut<'conn, 'param> {
    /// Convert this target into a parameter.
    fn into_bind_param_in_out(self) -> BindParam<'conn, 'param>;
}

/// Create an input parameter from a supported Rust value.
///
/// `Option::<T>::None` binds a typed SQL `NULL`; the type annotation is needed
/// when Rust cannot infer `T`.
///
/// The supported Rust-to-SQL type mappings are:
///
/// | Rust input | YashanDB/YACLI type |
/// | --- | --- |
/// | `bool` / `Option<bool>` | `BOOL` |
/// | `i8` / `Option<i8>` | `TINYINT` |
/// | `i16` / `Option<i16>` | `SMALLINT` |
/// | `i32` / `Option<i32>` | `INTEGER` |
/// | `i64` / `Option<i64>` | `BIGINT` |
/// | `f32` / `Option<f32>` | `FLOAT` |
/// | `f64` / `Option<f64>` | `DOUBLE` |
/// | [`crate::Number`] / `Option<`[`crate::Number`]`>` | `NUMBER` |
/// | [`crate::Date`] / `Option<`[`crate::Date`]`>` | `DATE` |
/// | [`crate::Time`] / `Option<`[`crate::Time`]`>` | `SHORTTIME` |
/// | [`crate::Timestamp`] / `Option<`[`crate::Timestamp`]`>` | `TIMESTAMP` |
/// | [`crate::IntervalYM`] / `Option<`[`crate::IntervalYM`]`>` | `INTERVAL YEAR TO MONTH` |
/// | [`crate::IntervalDS`] / `Option<`[`crate::IntervalDS`]`>` | `INTERVAL DAY TO SECOND` |
/// | `&str` / `String` and their `Option<T>` forms | `VARCHAR` |
/// | `&[u8]` / `Vec<u8>` and their `Option<T>` forms | `BINARY` |
/// | `&`[`crate::Blob`] / `Option<&`[`crate::Blob`]`>` | `BLOB` |
/// | `&`[`crate::Clob`] / `Option<&`[`crate::Clob`]`>` | `CLOB` |
///
/// `Option<T>` preserves the type inferred from `T` and uses SQL `NULL` when
/// the value is `None`.
#[inline]
pub fn input<'conn, 'param>(value: impl IntoBindParamIn<'conn, 'param>) -> BindParam<'conn, 'param> {
    value.into_bind_param_in()
}

/// Create an output parameter that the client writes after execution.
///
/// The target must remain valid for the duration of the call. For
/// `String`/`Vec<u8>`, initialize capacity before binding. For nullable
/// variable-size output, use `output((&mut value, capacity))`; `capacity` is a
/// minimum, and an existing larger allocation may be reused.
///
/// If execution returns an error, output targets may already contain data
/// written by the client. Do not rely on output target updates being atomic.
///
/// The supported Rust-to-SQL type mappings are:
///
/// | Rust output target | YashanDB/YACLI type |
/// | --- | --- |
/// | `&mut bool` / `&mut Option<bool>` | `BOOL` |
/// | `&mut i8` / `&mut Option<i8>` | `TINYINT` |
/// | `&mut i16` / `&mut Option<i16>` | `SMALLINT` |
/// | `&mut i32` / `&mut Option<i32>` | `INTEGER` |
/// | `&mut i64` / `&mut Option<i64>` | `BIGINT` |
/// | `&mut f32` / `&mut Option<f32>` | `FLOAT` |
/// | `&mut f64` / `&mut Option<f64>` | `DOUBLE` |
/// | `&mut `[`crate::Number`] / `&mut Option<`[`crate::Number`]`>` | `NUMBER` |
/// | `&mut `[`crate::Date`] / `&mut Option<`[`crate::Date`]`>` | `DATE` |
/// | `&mut `[`crate::Time`] / `&mut Option<`[`crate::Time`]`>` | `SHORTTIME` |
/// | `&mut `[`crate::Timestamp`] / `&mut Option<`[`crate::Timestamp`]`>` | `TIMESTAMP` |
/// | `&mut `[`crate::IntervalYM`] / `&mut Option<`[`crate::IntervalYM`]`>` | `INTERVAL YEAR TO MONTH` |
/// | `&mut `[`crate::IntervalDS`] / `&mut Option<`[`crate::IntervalDS`]`>` | `INTERVAL DAY TO SECOND` |
/// | `&mut String` | `VARCHAR` |
/// | `&mut Vec<u8>` | `BINARY` |
/// | `(&mut Option<String>, usize)` | nullable `VARCHAR` with minimum buffer capacity |
/// | `(&mut Option<Vec<u8>>, usize)` | nullable `BINARY` with minimum buffer capacity |
/// | `&mut `[`crate::Blob`] | `BLOB` |
/// | `&mut `[`crate::Clob`] | `CLOB` |
/// | `&mut Option<`[`crate::Blob`]`>` | nullable `BLOB` |
/// | `&mut Option<`[`crate::Clob`]`>` | nullable `CLOB` |
///
/// `Option<T>` targets receive `None` when the database returns SQL `NULL`.
/// `String` and `Vec<u8>` use their existing capacity as the output limit.
/// For a pure LOB output, use `&mut Option<crate::Blob>` or
/// `&mut Option<crate::Clob>`; binding allocates only a client descriptor and
/// does not create a temporary server LOB.
/// For a non-nullable pure LOB output, use [`crate::Connection::output_blob`]
/// or [`crate::Connection::output_clob`] and bind the result with `output`.
///
#[inline]
pub fn output<'conn, 'param>(target: impl IntoBindParamOut<'conn, 'param>) -> BindParam<'conn, 'param> {
    target.into_bind_param_out()
}

/// Create an input/output parameter.
///
/// The target is read before execution and updated after execution. For
/// `String`/`Vec<u8>`, the existing capacity limits the output size. Nullable
/// variable-size targets use a minimum buffer capacity and may reuse an
/// existing larger allocation.
///
/// If execution returns an error, input/output targets may already contain data
/// written by the client. Do not rely on target updates being atomic.
///
/// The supported Rust-to-SQL mappings are the same as [`output`]. The target
/// is read before execution and written after execution, so a fixed-size target
/// such as `&mut i64` maps to `BIGINT`, while `&mut Option<i64>` maps to the
/// nullable `BIGINT` form. Text and binary targets use `VARCHAR` and `BINARY`;
/// nullable variable-size targets use `(&mut Option<String>, usize)` or
/// `(&mut Option<Vec<u8>>, usize)` to provide a minimum buffer capacity.
/// LOB input/output parameters are not supported. Use [`input`] or [`output`]
/// for [`crate::Blob`] and [`crate::Clob`].
#[inline]
pub fn in_out<'conn, 'param>(target: impl IntoBindParamInOut<'conn, 'param>) -> BindParam<'conn, 'param> {
    target.into_bind_param_in_out()
}

impl<'conn, 'param> BindParam<'conn, 'param> {
    pub(crate) fn binding(&mut self, conn: &'conn Connection) -> Result<ParameterBinding<'_>, Error> {
        let (ext_type, value) = match &mut self.value {
            Value::Input(value) => {
                let (ty, bytes) = value.native_value();
                (ty, ParameterValue::Input(bytes))
            }
            Value::Output(value) => {
                let (ty, bytes) = value.native_value(conn)?;
                (ty, ParameterValue::Output(bytes))
            }
            Value::InOut(value) => {
                let (ty, bytes) = value.native_value(conn)?;
                (ty, ParameterValue::InOut(bytes))
            }
        };

        Ok(ParameterBinding {
            ext_type,
            value,
            indicator: &mut self.indicator,
        })
    }

    #[inline]
    pub(crate) fn complete(&mut self) -> Result<(), Error> {
        match &mut self.value {
            Value::Input(_) => Ok(()),
            Value::Output(value) | Value::InOut(value) => value.complete(self.indicator),
        }
    }
}

#[inline]
fn bytes_of<T>(value: &mut T) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut((value as *mut T).cast(), size_of::<T>()) }
}

#[inline]
fn indicator(len: usize) -> i32 {
    assert!(len <= i32::MAX as usize, "parameter length exceeds i32::MAX");
    len as i32
}
