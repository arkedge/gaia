mod field;
mod helper_impl;
mod macros;
pub mod util;
mod value;

use std::marker::PhantomData;

pub use field::{
    FloatingField, GenericFloatingField, GenericIntegralField, IntegralField, NumericField,
    SizedField,
};
pub use value::{FloatingValue, IntegralValue, NumericValue};

#[derive(Default)]
pub struct SizedBuilder<E, F> {
    fields: E,
    bit_len: usize,
    _field: PhantomData<*const F>,
}

impl<E, F> SizedBuilder<E, F> {
    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    pub fn byte_len(&self) -> usize {
        if self.bit_len() == 0 {
            0
        } else {
            (self.bit_len() - 1) / 8 + 1
        }
    }

    pub fn build(self) -> E {
        self.fields
    }
}

impl<E, F> Extend<F> for SizedBuilder<E, F>
where
    E: Extend<F>,
    F: SizedField,
{
    fn extend<T: IntoIterator<Item = F>>(&mut self, iter: T) {
        for field in iter {
            self.bit_len = self.bit_len.max(field.last_bit_exclusive());
            self.fields.extend([field]);
        }
    }
}
