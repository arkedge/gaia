use anyhow::{anyhow, ensure};

use crate::{macros::define_casting_integral, FloatingValue, IntegralValue, NumericValue};

define_casting_integral!(i8, I8);
define_casting_integral!(i16, I16);
define_casting_integral!(i32, I32);
define_casting_integral!(i64, I64);
define_casting_integral!(u8, U8);
define_casting_integral!(u16, U16);
define_casting_integral!(u32, U32);
define_casting_integral!(u64, U64);

impl TryFrom<NumericValue> for IntegralValue {
    type Error = anyhow::Error;

    fn try_from(value: NumericValue) -> Result<Self, Self::Error> {
        match value {
            NumericValue::Integral(i) => Ok(i),
            _ => Err(anyhow::anyhow!(
                "cannot convert floating point number to integral number"
            )),
        }
    }
}

impl From<f32> for FloatingValue {
    fn from(f: f32) -> Self {
        Self::F32(f)
    }
}

impl From<f64> for FloatingValue {
    fn from(f: f64) -> Self {
        Self::F64(f)
    }
}

impl TryFrom<FloatingValue> for f32 {
    type Error = anyhow::Error;

    fn try_from(value: FloatingValue) -> Result<Self, Self::Error> {
        match value {
            FloatingValue::F32(f) => Ok(f),
            FloatingValue::F64(d) => {
                let f = d as f32;
                ensure!(f.is_finite() || d.is_infinite(), "overflow: {} for f32", d);
                Ok(f)
            }
        }
    }
}
impl TryFrom<FloatingValue> for f64 {
    type Error = anyhow::Error;

    fn try_from(value: FloatingValue) -> Result<Self, Self::Error> {
        match value {
            FloatingValue::F32(f) => Ok(f as f64),
            FloatingValue::F64(d) => Ok(d),
        }
    }
}

impl TryFrom<NumericValue> for FloatingValue {
    type Error = anyhow::Error;

    fn try_from(value: NumericValue) -> Result<Self, Self::Error> {
        match value {
            NumericValue::Floating(f) => Ok(f),
            _ => Err(anyhow!(
                "cannot convert integral number to floating point number"
            )),
        }
    }
}

impl From<IntegralValue> for NumericValue {
    fn from(i: IntegralValue) -> Self {
        Self::Integral(i)
    }
}

impl From<FloatingValue> for NumericValue {
    fn from(f: FloatingValue) -> Self {
        Self::Floating(f)
    }
}
