//! Statement allocation, execution, metadata, binding, and fetch operations.

use std::ffi::CStr;
use std::ffi::c_void;
use std::ptr::NonNull;

use crate::error::Error;
use crate::ffi::raw::*;
use crate::ffi::{DbcHandle, YacLib};

#[repr(transparent)]
pub struct StmtHandle(NonNull<c_void>);

pub enum ParameterValue<'a> {
    Input(&'a [u8]),
    Output(&'a mut [u8]),
    InOut(&'a mut [u8]),
}

pub struct ParameterBinding<'a> {
    pub ext_type: YacExtType,
    pub value: ParameterValue<'a>,
    pub indicator: &'a mut i32,
}

impl YacLib {
    #[inline]
    pub fn alloc_stmt(&self, dbc: &mut DbcHandle) -> Result<StmtHandle, Error> {
        let mut output = std::ptr::null_mut();
        self.try_call(|| unsafe { (self.alloc_handle)(YacHandleType::Stmt, dbc.as_ptr(), &mut output) })?;
        NonNull::new(output)
            .map(StmtHandle)
            .ok_or_else(|| Error::ClientLibrary("yacAllocHandle returned a null stmt handle".to_string()))
    }

    #[inline]
    pub fn free_stmt(&self, stmt: &mut StmtHandle) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.free_handle)(YacHandleType::Stmt, stmt.0.as_ptr()) })
    }

    #[inline]
    pub fn direct_execute(&self, stmt: &mut StmtHandle, sql: &str) -> Result<(), Error> {
        assert!(sql.len() <= i32::MAX as usize);
        let len = sql.len() as i32;
        self.try_call(|| unsafe { (self.direct_execute)(stmt.0.as_ptr(), sql.as_ptr(), len) })
    }

    #[inline]
    pub fn prepare(&self, stmt: &mut StmtHandle, sql: &str) -> Result<(), Error> {
        assert!(sql.len() <= i32::MAX as usize);
        let len = sql.len() as i32;
        self.try_call(|| unsafe { (self.prepare)(stmt.0.as_ptr(), sql.as_ptr(), len) })
    }

    #[inline]
    pub fn execute(&self, stmt: &mut StmtHandle) -> Result<(), Error> {
        self.try_call(|| unsafe { (self.execute)(stmt.0.as_ptr()) })
    }

    #[inline]
    pub fn num_params(&self, stmt: &StmtHandle) -> Result<u16, Error> {
        let mut count = 0;
        self.try_call(|| unsafe { (self.num_params)(stmt.0.as_ptr(), &mut count) })?;
        Ok(count)
    }

    #[inline]
    pub fn bind_parameter(&self, stmt: &mut StmtHandle, id: u16, binding: ParameterBinding<'_>) -> Result<(), Error> {
        let ParameterBinding {
            ext_type,
            value,
            indicator,
        } = binding;
        let (direction, value, bind_size) = parameter_value(value);
        self.try_call(|| unsafe {
            (self.bind_parameter)(
                stmt.0.as_ptr(),
                id,
                direction,
                ext_type as u32,
                value,
                bind_size,
                bind_size,
                indicator,
            )
        })
    }

    #[inline]
    pub fn bind_parameter_by_name(
        &self,
        stmt: &mut StmtHandle,
        name: &CStr,
        binding: ParameterBinding<'_>,
    ) -> Result<(), Error> {
        let ParameterBinding {
            ext_type,
            value,
            indicator,
        } = binding;
        let (direction, value, bind_size) = parameter_value(value);
        self.try_call(|| unsafe {
            (self.bind_parameter_by_name)(
                stmt.0.as_ptr(),
                name.as_ptr().cast_mut().cast(),
                direction,
                ext_type as u32,
                value,
                bind_size,
                bind_size,
                indicator,
            )
        })
    }

    #[inline]
    pub fn get_stmt_rows_affected(&self, stmt: &StmtHandle) -> Result<u64, Error> {
        let mut rows: YacUint64 = 0;
        let mut length = 0;
        self.try_call(|| unsafe {
            (self.get_stmt_attr)(
                stmt.0.as_ptr(),
                YacStmtAttr::RowsAffected,
                (&mut rows as *mut u64).cast(),
                size_of::<u64>() as i32,
                &mut length,
            )
        })?;
        Ok(rows)
    }

    #[inline]
    pub fn get_num_result_cols(&self, stmt: &StmtHandle) -> Result<u16, Error> {
        let mut count = 0;
        self.try_call(|| unsafe { (self.num_result_cols)(stmt.0.as_ptr(), &mut count) })?;
        assert!(count >= 0);
        Ok(count as u16)
    }

    #[inline]
    fn get_stmt_col_bytes(
        &self,
        stmt: &StmtHandle,
        index: u16,
        attr: YacColAttr,
        buffer: &mut [u8],
    ) -> Result<u32, Error> {
        let mut length = 0;
        self.try_call(|| unsafe {
            (self.col_attribute)(
                stmt.0.as_ptr(),
                index,
                attr,
                buffer.as_mut_ptr().cast(),
                buffer.len() as i32,
                &mut length,
            )
        })?;
        assert!(length >= 0);
        Ok(length as u32)
    }

    #[inline]
    pub fn get_stmt_col_name(&self, stmt: &StmtHandle, index: u16, buffer: &mut [u8]) -> Result<u32, Error> {
        self.get_stmt_col_bytes(stmt, index, YacColAttr::Name, buffer)
    }

    #[inline]
    fn get_stmt_col_u32(&self, stmt: &StmtHandle, index: u16, attr: YacColAttr) -> Result<u32, Error> {
        let mut value = 0_u32;
        let mut length = 0;
        self.try_call(|| unsafe {
            (self.col_attribute)(
                stmt.0.as_ptr(),
                index,
                attr,
                (&mut value as *mut u32).cast(),
                size_of::<u32>() as i32,
                &mut length,
            )
        })?;
        Ok(value)
    }

    #[inline]
    fn get_stmt_col_u8(&self, stmt: &StmtHandle, index: u16, attr: YacColAttr) -> Result<u8, Error> {
        let mut value = 0_u8;
        let mut length = 0;
        self.try_call(|| unsafe {
            (self.col_attribute)(
                stmt.0.as_ptr(),
                index,
                attr,
                (&mut value as *mut u8).cast(),
                size_of::<u8>() as i32,
                &mut length,
            )
        })?;
        Ok(value)
    }

    #[inline]
    fn get_stmt_col_i8(&self, stmt: &StmtHandle, index: u16, attr: YacColAttr) -> Result<i8, Error> {
        let mut value = 0_i8;
        let mut length = 0;
        self.try_call(|| unsafe {
            (self.col_attribute)(
                stmt.0.as_ptr(),
                index,
                attr,
                (&mut value as *mut i8).cast(),
                size_of::<i8>() as i32,
                &mut length,
            )
        })?;
        Ok(value)
    }

    #[inline]
    pub fn get_stmt_col_size(&self, stmt: &StmtHandle, index: u16) -> Result<u32, Error> {
        self.get_stmt_col_u32(stmt, index, YacColAttr::Size)
    }

    #[inline]
    pub fn get_stmt_col_type(&self, stmt: &StmtHandle, index: u16) -> Result<YacType, Error> {
        let mut value = YacType::Unknown;
        let mut length = 0;
        self.try_call(|| unsafe {
            (self.col_attribute)(
                stmt.0.as_ptr(),
                index,
                YacColAttr::Type,
                (&mut value as *mut YacType).cast(),
                size_of::<YacType>() as i32,
                &mut length,
            )
        })?;
        Ok(value)
    }

    #[inline]
    pub fn get_stmt_col_precision(&self, stmt: &StmtHandle, index: u16) -> Result<u8, Error> {
        self.get_stmt_col_u8(stmt, index, YacColAttr::Precision)
    }

    #[inline]
    pub fn get_stmt_col_nullable(&self, stmt: &StmtHandle, index: u16) -> Result<bool, Error> {
        self.get_stmt_col_u8(stmt, index, YacColAttr::Nullable)
            .map(|value| value != 0)
    }

    #[inline]
    pub fn get_stmt_col_char_size(&self, stmt: &StmtHandle, index: u16) -> Result<u32, Error> {
        self.get_stmt_col_u32(stmt, index, YacColAttr::CharSize)
    }

    #[inline]
    pub fn get_stmt_col_scale(&self, stmt: &StmtHandle, index: u16) -> Result<i8, Error> {
        self.get_stmt_col_i8(stmt, index, YacColAttr::Scale)
    }

    #[inline]
    pub fn bind_column(
        &self,
        stmt: &mut StmtHandle,
        index: u16,
        ext_type: YacExtType,
        buffer: &mut [u8],
        indicator: &mut i32,
    ) -> Result<(), Error> {
        assert!(buffer.len() <= i32::MAX as usize);
        let len = buffer.len() as i32;
        self.try_call(|| unsafe {
            (self.bind_column)(
                stmt.0.as_ptr(),
                index,
                ext_type as u32,
                buffer.as_mut_ptr().cast::<c_void>(),
                len,
                indicator,
            )
        })
    }

    #[inline]
    pub fn fetch(&self, stmt: &mut StmtHandle) -> Result<u32, Error> {
        let mut rows = 0;
        self.try_call(|| unsafe { (self.fetch)(stmt.0.as_ptr(), &mut rows) })?;
        Ok(rows)
    }
}

#[inline]
fn parameter_value(value: ParameterValue<'_>) -> (YacParamDirection, *mut c_void, i32) {
    let (direction, ptr, len) = match value {
        ParameterValue::Input(value) => (YacParamDirection::Input, value.as_ptr().cast_mut().cast(), value.len()),
        ParameterValue::Output(value) => (YacParamDirection::Output, value.as_mut_ptr().cast(), value.len()),
        ParameterValue::InOut(value) => (YacParamDirection::InOut, value.as_mut_ptr().cast(), value.len()),
    };
    assert!(len <= i32::MAX as usize);
    (direction, ptr, len as i32)
}
