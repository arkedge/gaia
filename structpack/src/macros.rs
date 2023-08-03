macro_rules! define_casting_integral {
    ($ty:ident, $variant:ident) => {
        impl TryFrom<$crate::IntegralValue> for $ty {
            type Error = anyhow::Error;

            fn try_from(value: $crate::IntegralValue) -> Result<Self, Self::Error> {
                Ok(match value {
                    $crate::IntegralValue::I8(v) => v.try_into()?,
                    $crate::IntegralValue::I16(v) => v.try_into()?,
                    $crate::IntegralValue::I32(v) => v.try_into()?,
                    $crate::IntegralValue::I64(v) => v.try_into()?,
                    $crate::IntegralValue::U8(v) => v.try_into()?,
                    $crate::IntegralValue::U16(v) => v.try_into()?,
                    $crate::IntegralValue::U32(v) => v.try_into()?,
                    $crate::IntegralValue::U64(v) => v.try_into()?,
                })
            }
        }

        impl From<$ty> for $crate::IntegralValue {
            fn from(v: $ty) -> Self {
                Self::$variant(v)
            }
        }
    };
}

pub(crate) use define_casting_integral;
