//! Public database value types and internal client value layouts.

/// Day-to-second interval value.
pub use sqldatetime::IntervalDS;
/// Year-to-month interval value.
pub use sqldatetime::IntervalYM;
/// Calendar date value.
pub use sqldatetime::OracleDate as Date;
/// Time-of-day value.
pub use sqldatetime::Time;
/// Timestamp without a time zone.
pub use sqldatetime::Timestamp;

/// Database NUMBER value.
///
/// This representation supports up to 38 significant decimal digits.
pub use decimal_rs::Decimal as Number;

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub(crate) struct YacNumber {
    value: u128,
    sign: i8,
    _unused: u8,
    exp: i16,
}

impl YacNumber {
    pub const ZERO: YacNumber = YacNumber {
        value: 0,
        sign: 0,
        _unused: 0,
        exp: 0,
    };

    #[inline]
    pub const fn into_number(self) -> Number {
        // YacNumber's exponent has the opposite sign from Number's scale.
        unsafe { Number::from_parts_unchecked(self.value, -self.exp, self.sign != 0) }
    }

    #[inline]
    pub const fn from_number(number: Number) -> Self {
        let (value, scale, negative) = number.into_parts();
        Self {
            value,
            sign: negative as i8,
            _unused: 0,
            exp: -scale,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
pub(crate) struct YacTimestamp {
    pub timestamp: Timestamp,
    pub bias: i16,
    _unused: i16,
}

impl YacTimestamp {
    #[inline]
    pub const fn with_timestamp(timestamp: Timestamp) -> Self {
        Self {
            timestamp,
            bias: 0,
            _unused: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::YacNumber;

    #[test]
    fn yac_number_layout() {
        assert_eq!(size_of::<YacNumber>(), 20);
        assert_eq!(align_of::<YacNumber>(), 4);
    }
}
