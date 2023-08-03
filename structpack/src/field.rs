use std::{marker::PhantomData, ops::Range};

use anyhow::{anyhow, Result};
use bitvec::{field::BitField, prelude::Msb0, view::BitView};
use funty::{Floating, Integral};
use serde::{Deserialize, Serialize};

use crate::{FloatingValue, IntegralValue, NumericValue};

pub trait SizedField {
    type Value<'a>;
    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>>;
    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> Result<()>;
    fn last_bit_exclusive(&self) -> usize;
    fn bit_len(&self) -> usize;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegralField<T> {
    range: Range<usize>,
    #[serde(skip)]
    _type: PhantomData<fn() -> T>,
}
impl<T> IntegralField<T>
where
    T: Integral,
{
    pub fn new(range: Range<usize>) -> Option<Self> {
        if range.is_empty() || range.len() > T::BITS as usize {
            return None;
        }
        Some(Self {
            range,
            _type: PhantomData,
        })
    }

    fn err_out_of_range(&self, bytes_len: usize) -> anyhow::Error {
        anyhow!(
            "field is out of range: {:?} bits for {} bytes",
            self.range,
            bytes_len
        )
    }
}

impl<T> SizedField for IntegralField<T>
where
    T: Integral,
{
    type Value<'a> = T;

    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>> {
        let slice = bytes
            .view_bits::<Msb0>()
            .get(self.range.clone())
            .ok_or_else(|| self.err_out_of_range(bytes.len()))?;
        Ok(slice.load_be())
    }

    fn write<'a>(&self, bytes: &'a mut [u8], value: Self::Value<'a>) -> Result<()> {
        let len = bytes.len();
        let slice = bytes
            .view_bits_mut::<Msb0>()
            .get_mut(self.range.clone())
            .ok_or_else(|| self.err_out_of_range(len))?;
        slice.store_be(value);
        Ok(())
    }

    fn last_bit_exclusive(&self) -> usize {
        self.range.end
    }

    fn bit_len(&self) -> usize {
        self.range.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GenericIntegralField {
    I8(IntegralField<i8>),
    I16(IntegralField<i16>),
    I32(IntegralField<i32>),
    I64(IntegralField<i64>),
    U8(IntegralField<u8>),
    U16(IntegralField<u16>),
    U32(IntegralField<u32>),
    U64(IntegralField<u64>),
}

impl SizedField for GenericIntegralField {
    type Value<'a> = IntegralValue;

    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>> {
        Ok(match self {
            Self::I8(f) => f.read(bytes)?.into(),
            Self::I16(f) => f.read(bytes)?.into(),
            Self::I32(f) => f.read(bytes)?.into(),
            Self::I64(f) => f.read(bytes)?.into(),
            Self::U8(f) => f.read(bytes)?.into(),
            Self::U16(f) => f.read(bytes)?.into(),
            Self::U32(f) => f.read(bytes)?.into(),
            Self::U64(f) => f.read(bytes)?.into(),
        })
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> Result<()> {
        match self {
            Self::I8(f) => f.write(bytes, value.try_into()?),
            Self::I16(f) => f.write(bytes, value.try_into()?),
            Self::I32(f) => f.write(bytes, value.try_into()?),
            Self::I64(f) => f.write(bytes, value.try_into()?),
            Self::U8(f) => f.write(bytes, value.try_into()?),
            Self::U16(f) => f.write(bytes, value.try_into()?),
            Self::U32(f) => f.write(bytes, value.try_into()?),
            Self::U64(f) => f.write(bytes, value.try_into()?),
        }
    }

    fn last_bit_exclusive(&self) -> usize {
        match self {
            Self::I8(f) => f.last_bit_exclusive(),
            Self::I16(f) => f.last_bit_exclusive(),
            Self::I32(f) => f.last_bit_exclusive(),
            Self::I64(f) => f.last_bit_exclusive(),
            Self::U8(f) => f.last_bit_exclusive(),
            Self::U16(f) => f.last_bit_exclusive(),
            Self::U32(f) => f.last_bit_exclusive(),
            Self::U64(f) => f.last_bit_exclusive(),
        }
    }

    fn bit_len(&self) -> usize {
        match self {
            Self::I8(f) => f.bit_len(),
            Self::I16(f) => f.bit_len(),
            Self::I32(f) => f.bit_len(),
            Self::I64(f) => f.bit_len(),
            Self::U8(f) => f.bit_len(),
            Self::U16(f) => f.bit_len(),
            Self::U32(f) => f.bit_len(),
            Self::U64(f) => f.bit_len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingField<T> {
    range: Range<usize>,
    #[serde(skip)]
    _type: PhantomData<fn() -> T>,
}
impl<T> FloatingField<T>
where
    T: Floating,
{
    pub fn new(range: Range<usize>) -> Option<Self> {
        if range.is_empty() || range.len() > T::Raw::BITS as usize {
            return None;
        }
        Some(Self {
            range,
            _type: PhantomData,
        })
    }

    fn err_out_of_range(&self, bytes_len: usize) -> anyhow::Error {
        anyhow!(
            "field is out of range: {:?} bits for {} bytes",
            self.range,
            bytes_len
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GenericFloatingField {
    F32(FloatingField<f32>),
    F64(FloatingField<f64>),
}

impl SizedField for GenericFloatingField {
    type Value<'a> = FloatingValue;

    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>> {
        Ok(match self {
            Self::F32(f) => f.read(bytes)?.into(),
            Self::F64(f) => f.read(bytes)?.into(),
        })
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> Result<()> {
        match self {
            Self::F32(f) => f.write(bytes, value.try_into()?),
            Self::F64(f) => f.write(bytes, value.try_into()?),
        }
    }

    fn last_bit_exclusive(&self) -> usize {
        match self {
            Self::F32(f) => f.last_bit_exclusive(),
            Self::F64(f) => f.last_bit_exclusive(),
        }
    }

    fn bit_len(&self) -> usize {
        match self {
            Self::F32(f) => f.bit_len(),
            Self::F64(f) => f.bit_len(),
        }
    }
}

impl<T> SizedField for FloatingField<T>
where
    T: Floating,
{
    type Value<'a> = T;

    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>> {
        let slice = bytes
            .view_bits::<Msb0>()
            .get(self.range.clone())
            .ok_or_else(|| self.err_out_of_range(bytes.len()))?;
        Ok(T::from_bits(slice.load_be()))
    }

    fn write<'a>(&self, bytes: &'a mut [u8], value: Self::Value<'a>) -> Result<()> {
        let len = bytes.len();
        let slice = bytes
            .view_bits_mut::<Msb0>()
            .get_mut(self.range.clone())
            .ok_or_else(|| self.err_out_of_range(len))?;
        slice.store_be(value.to_bits());
        Ok(())
    }

    fn last_bit_exclusive(&self) -> usize {
        self.range.end
    }

    fn bit_len(&self) -> usize {
        self.range.len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NumericField {
    Integral(GenericIntegralField),
    Floating(GenericFloatingField),
}

impl SizedField for NumericField {
    type Value<'a> = NumericValue;

    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>> {
        Ok(match self {
            Self::Integral(i) => i.read(bytes)?.into(),
            Self::Floating(f) => f.read(bytes)?.into(),
        })
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> Result<()> {
        match self {
            Self::Integral(i) => i.write(bytes, value.try_into()?),
            Self::Floating(f) => f.write(bytes, value.try_into()?),
        }
    }

    fn last_bit_exclusive(&self) -> usize {
        match self {
            Self::Integral(i) => i.last_bit_exclusive(),
            Self::Floating(f) => f.last_bit_exclusive(),
        }
    }

    fn bit_len(&self) -> usize {
        match self {
            Self::Integral(i) => i.bit_len(),
            Self::Floating(f) => f.bit_len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        const DEADBEEF: &[u8] = [0xDE, 0xAD, 0xBE, 0xEF].as_slice();
        let de_u8 = IntegralField::<u8>::new(0..8).unwrap();
        let adbe_u16 = IntegralField::<u16>::new(8..24).unwrap();
        assert_eq!(de_u8.read(DEADBEEF).unwrap(), 0xDE);
        assert_eq!(adbe_u16.read(DEADBEEF).unwrap(), 0xADBE);
        let n = GenericIntegralField::U8(de_u8);
        let u: u8 = n.read(DEADBEEF).unwrap().try_into().unwrap();
        assert_eq!(u, 0xDE);
        let i: Result<i8> = n.read(DEADBEEF).unwrap().try_into();
        assert!(i.is_err());
    }

    #[test]
    fn test_f64() {
        let double: [u8; 8] = (789.456f64).to_be_bytes();
        let slice = &double[..];
        let f64_field = FloatingField::<f64>::new(0..64).unwrap();
        assert_eq!(f64_field.read(slice).unwrap(), 789.456);
    }
}
