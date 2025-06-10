use anyhow::{ensure, Result};
use funty::{Floating, Integral};
use structpack::{
    FloatingField, GenericFloatingField, GenericIntegralField, IntegralField, NumericField,
    NumericValue, SizedField,
};
use tlmcmddb::{cmd as cmddb, Component};

use super::Writer;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub component_name: String,
    pub command_name: String,
    pub cmd_id: u16,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct CommandSchema {
    pub sized_parameters: Vec<ParameterField>,
    pub static_size: usize,
    pub has_trailer_parameter: bool,
}

#[derive(Debug, Clone)]
pub struct ParameterField {
    pub value: NumericField,
    pub description: String,
}

impl SizedField for ParameterField {
    type Value<'a> = NumericValue;

    fn read<'a>(&self, bytes: &'a [u8]) -> Result<Self::Value<'a>> {
        self.value.read(bytes)
    }

    fn write(&self, bytes: &mut [u8], value: Self::Value<'_>) -> Result<()> {
        self.value.write(bytes, value)
    }

    fn last_bit_exclusive(&self) -> usize {
        self.value.last_bit_exclusive()
    }

    fn bit_len(&self) -> usize {
        self.value.bit_len()
    }
}

impl CommandSchema {
    pub fn build_writer<'b>(
        &'b self,
        bytes: &'b mut [u8],
    ) -> Writer<'b, std::slice::Iter<'b, ParameterField>> {
        Writer::new(
            self.sized_parameters.iter(),
            self.static_size,
            self.has_trailer_parameter,
            bytes,
        )
    }
}

pub fn from_tlmcmddb(db: &tlmcmddb::Database) -> ComponentIter {
    ComponentIter {
        iter: db.components.iter(),
    }
}

pub struct ComponentIter<'a> {
    iter: std::slice::Iter<'a, Component>,
}

impl<'a> Iterator for ComponentIter<'a> {
    type Item = Iter<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let component = self.iter.next()?;
        Some(Iter {
            name: &component.name,
            entries: component.cmd.entries.iter(),
        })
    }
}

pub struct Iter<'a> {
    name: &'a str,
    entries: std::slice::Iter<'a, cmddb::Entry>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Result<(Metadata, CommandSchema)>;

    fn next(&mut self) -> Option<Self::Item> {
        #[allow(clippy::never_loop)]
        loop {
            let cmddb::Entry::Command(command) = self.entries.next()? else {
                continue;
            };
            let metadata = Metadata {
                component_name: self.name.to_string(),
                command_name: command.name.to_string(),
                cmd_id: command.code,
                description: command.description.to_string(),
            };
            return build_schema(command)
                .map(|schema| Some((metadata, schema)))
                .transpose();
        }
    }
}

fn build_schema(db: &cmddb::Command) -> Result<CommandSchema> {
    let mut params_iter = db.parameters.iter();
    let mut static_size_bits = 0;
    let mut sized_parameters = vec![];
    let mut has_trailer_parameter = false;
    for parameter in params_iter.by_ref() {
        if let Some(field) = build_numeric_field(static_size_bits, parameter) {
            static_size_bits += field.bit_len();
            sized_parameters.push(ParameterField {
                value: field,
                description: parameter.description.clone(),
            });
        } else {
            // raw parameter is present
            has_trailer_parameter = true;
            break;
        }
    }
    ensure!(
        params_iter.next().is_none(),
        "trailer(RAW) parameter is valid only if at the last position"
    );
    let static_size = if static_size_bits == 0 {
        0
    } else {
        (static_size_bits - 1) / 8 + 1
    };
    Ok(CommandSchema {
        sized_parameters,
        static_size,
        has_trailer_parameter,
    })
}

fn build_numeric_field(offset: usize, parameter: &cmddb::Parameter) -> Option<NumericField> {
    match parameter.data_type {
        cmddb::DataType::Int8 => Some(NumericField::Integral(GenericIntegralField::I8(
            build_command_integral_field(offset, 8),
        ))),
        cmddb::DataType::Int16 => Some(NumericField::Integral(GenericIntegralField::I16(
            build_command_integral_field(offset, 16),
        ))),
        cmddb::DataType::Int32 => Some(NumericField::Integral(GenericIntegralField::I32(
            build_command_integral_field(offset, 32),
        ))),
        cmddb::DataType::Uint8 => Some(NumericField::Integral(GenericIntegralField::U8(
            build_command_integral_field(offset, 8),
        ))),
        cmddb::DataType::Uint16 => Some(NumericField::Integral(GenericIntegralField::U16(
            build_command_integral_field(offset, 16),
        ))),
        cmddb::DataType::Uint32 => Some(NumericField::Integral(GenericIntegralField::U32(
            build_command_integral_field(offset, 32),
        ))),
        cmddb::DataType::Float => Some(NumericField::Floating(GenericFloatingField::F32(
            build_command_floating_field(offset, 32),
        ))),
        cmddb::DataType::Double => Some(NumericField::Floating(GenericFloatingField::F64(
            build_command_floating_field(offset, 64),
        ))),
        cmddb::DataType::Raw => None,
    }
}

fn build_command_integral_field<T: Integral>(offset: usize, len: usize) -> IntegralField<T> {
    IntegralField::new(offset..offset + len).expect("never fails")
}

fn build_command_floating_field<T: Floating>(offset: usize, len: usize) -> FloatingField<T> {
    FloatingField::new(offset..offset + len).expect("never fails")
}
