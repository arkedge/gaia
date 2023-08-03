use serde::{Deserialize, Serialize};

use crate::SizedField;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldWithMetadata<M, F> {
    pub metadata: M,
    pub field: F,
}

impl<M, F> SizedField for FieldWithMetadata<M, F>
where
    F: SizedField,
{
    type Value<'a> = F::Value<'a>;

    fn read<'a>(&self, bytes: &'a [u8]) -> anyhow::Result<Self::Value<'a>> {
        self.field.read(bytes)
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> anyhow::Result<()> {
        self.field.write(bytes, value)
    }

    fn last_bit_exclusive(&self) -> usize {
        self.field.last_bit_exclusive()
    }

    fn bit_len(&self) -> usize {
        self.field.bit_len()
    }
}

impl<M, F> SizedField for (M, F)
where
    F: SizedField,
{
    type Value<'a> = F::Value<'a>;

    fn read<'a>(&self, bytes: &'a [u8]) -> anyhow::Result<Self::Value<'a>> {
        self.1.read(bytes)
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> anyhow::Result<()> {
        self.1.write(bytes, value)
    }

    fn last_bit_exclusive(&self) -> usize {
        self.1.last_bit_exclusive()
    }

    fn bit_len(&self) -> usize {
        self.1.bit_len()
    }
}

impl<T, U, F> SizedField for (T, U, F)
where
    F: SizedField,
{
    type Value<'a> = F::Value<'a>;

    fn read<'a>(&self, bytes: &'a [u8]) -> anyhow::Result<Self::Value<'a>> {
        self.2.read(bytes)
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> anyhow::Result<()> {
        self.2.write(bytes, value)
    }

    fn last_bit_exclusive(&self) -> usize {
        self.2.last_bit_exclusive()
    }

    fn bit_len(&self) -> usize {
        self.2.bit_len()
    }
}
