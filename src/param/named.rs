//! Named prepared-statement parameters.

use std::borrow::Cow;
use std::ffi::CStr;

use super::BindParam;

/// A named parameter and its value.
///
/// The name must be a NUL-terminated [`CStr`]. Pass the name without the SQL
/// placeholder prefix: `value` corresponds to `:value` in SQL. Named
/// parameters are supplied as an array, slice, or `Vec`.
pub struct NamedBindParam<'name, 'param> {
    name: Cow<'name, CStr>,
    param: BindParam<'param>,
}

/// Associate a NUL-terminated parameter name with a parameter value.
///
/// The `name` argument is passed to the client unchanged. Use a
/// [`std::ffi::CString`]
/// for an owned name or a [`CStr`] for a borrowed name.
#[inline]
pub fn named<'name, 'param>(
    name: impl Into<Cow<'name, CStr>>,
    param: BindParam<'param>,
) -> NamedBindParam<'name, 'param> {
    NamedBindParam {
        name: name.into(),
        param,
    }
}

impl<'name, 'param> NamedBindParam<'name, 'param> {
    /// Parameter name.
    #[inline]
    pub fn name(&self) -> &CStr {
        &self.name
    }

    #[inline]
    pub(crate) fn param_mut(&mut self) -> &mut BindParam<'param> {
        &mut self.param
    }

    #[inline]
    pub(crate) fn parts_mut(&mut self) -> (&CStr, &mut BindParam<'param>) {
        (&self.name, &mut self.param)
    }
}
