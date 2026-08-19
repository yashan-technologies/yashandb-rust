//! Result-column index resolution.

use crate::column::ColumnInfo;
use crate::error::Error;

/// A column position or database-reported column name.
pub trait ColumnIndex {
    /// Resolve this index against result column metadata.
    fn index(&self, columns: &[ColumnInfo]) -> Result<usize, Error>;
}

impl ColumnIndex for usize {
    #[inline]
    fn index(&self, columns: &[ColumnInfo]) -> Result<usize, Error> {
        if *self < columns.len() {
            Ok(*self)
        } else {
            Err(Error::ColumnIndexOutOfBounds {
                index: *self,
                column_count: columns.len(),
            })
        }
    }
}

impl ColumnIndex for &str {
    #[inline]
    fn index(&self, columns: &[ColumnInfo]) -> Result<usize, Error> {
        columns
            .iter()
            .position(|info| info.name == *self)
            .ok_or_else(|| Error::ColumnNotFound {
                name: (*self).to_owned(),
            })
    }
}
