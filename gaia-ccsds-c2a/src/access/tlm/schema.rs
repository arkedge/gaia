use std::ops::Range;

use anyhow::{anyhow, ensure, Result};
use funty::{Floating, Integral};
use structpack::{
    FloatingField, FloatingValue, GenericFloatingField, GenericIntegralField, IntegralField,
    IntegralValue, SizedField,
};
use tlmcmddb::{tlm as tlmdb, Component};

use crate::access::tlm::converter;

use super::FieldValue;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub component_name: String,
    pub telemetry_name: String,
    pub tlm_id: u8,
    pub is_restriced: bool,
}

#[derive(Debug, Clone)]
pub struct IntegralFieldSchema {
    pub converter: Option<converter::Integral>,
    pub field: structpack::GenericIntegralField,
}

impl IntegralFieldSchema {
    pub fn read_from(&self, bytes: &[u8]) -> Result<(Vec<u8>, FieldValue)> {
        let value = self.field.read(bytes)?;
        let raw = integral_to_bytes(value.clone());
        let converted = match &self.converter {
            Some(converter::Integral::Status(status)) => {
                FieldValue::Constant(status.convert(value.try_into()?))
            }
            Some(converter::Integral::Polynomial(poly)) => {
                FieldValue::Double(poly.convert(try_integral_to_f64(value)?))
            }
            None => FieldValue::Integer(value.try_into()?),
        };
        Ok((raw, converted))
    }
}

#[derive(Debug, Clone)]
pub struct FloatingFieldSchema {
    pub converter: Option<converter::Polynomial>,
    pub field: structpack::GenericFloatingField,
}

impl FloatingFieldSchema {
    pub fn read_from(&self, bytes: &[u8]) -> Result<(Vec<u8>, FieldValue)> {
        let value = self.field.read(bytes)?;
        let raw = floating_to_bytes(value.clone());
        let converted = match &self.converter {
            Some(poly) => FieldValue::Double(poly.convert(value.try_into()?)),
            None => FieldValue::Double(value.try_into()?),
        };
        Ok((raw, converted))
    }
}

pub struct FieldSchema {
    pub metadata: FieldMetadata,
    pub value: FieldValueSchema,
}

pub struct FieldMetadata {
    pub description: String,
}

pub enum FieldValueSchema {
    Integral(IntegralFieldSchema),
    Floating(FloatingFieldSchema),
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
    type Item = TelemetryIter<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let component = self.iter.next()?;
        Some(TelemetryIter {
            component_name: &component.name,
            telemetries: component.tlm.telemetries.iter(),
        })
    }
}

pub struct TelemetryIter<'a> {
    component_name: &'a str,
    telemetries: std::slice::Iter<'a, tlmdb::Telemetry>,
}

impl<'a> Iterator for TelemetryIter<'a> {
    type Item = (Metadata, FieldIter<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let telemetry = self.telemetries.next()?;
        let metadata = Metadata {
            component_name: self.component_name.to_string(),
            telemetry_name: telemetry.name.to_string(),
            tlm_id: telemetry.metadata.packet_id,
            is_restriced: telemetry.metadata.is_restricted,
        };
        let fields = Box::new(iter_fields(&telemetry.entries).filter_map(|(obs, field)| {
            build_bit_range(&field.extraction_info).map(|bit_range| (obs, field, bit_range))
        }));
        Some((metadata, FieldIter { fields }))
    }
}

pub struct FieldIter<'a> {
    fields:
        Box<dyn Iterator<Item = (tlmdb::OnboardSoftwareInfo, &'a tlmdb::Field, Range<usize>)> + 'a>,
}

impl<'a> Iterator for FieldIter<'a> {
    type Item = Result<(&'a str, FieldSchema)>;

    fn next(&mut self) -> Option<Self::Item> {
        let (obs, field, bit_range) = self.fields.next()?;
        build_field_schema(obs, field, bit_range)
            .map(Some)
            .transpose()
    }
}

fn build_field_schema(
    obs: tlmdb::OnboardSoftwareInfo,
    field: &tlmdb::Field,
    bit_range: Range<usize>,
) -> Result<(&str, FieldSchema)> {
    let converter = build_integral_converter(&field.conversion_info);
    Ok((
        &field.name,
        FieldSchema {
            metadata: FieldMetadata {
                description: field.description.clone(),
            },
            value: match obs.variable_type {
                tlmdb::VariableType::Int8 => FieldValueSchema::Integral(IntegralFieldSchema {
                    converter,
                    field: GenericIntegralField::I8(build_telemetry_integral_field(bit_range)?),
                }),
                tlmdb::VariableType::Int16 => FieldValueSchema::Integral(IntegralFieldSchema {
                    converter,
                    field: GenericIntegralField::I16(build_telemetry_integral_field(bit_range)?),
                }),
                tlmdb::VariableType::Int32 => FieldValueSchema::Integral(IntegralFieldSchema {
                    converter,
                    field: GenericIntegralField::I32(build_telemetry_integral_field(bit_range)?),
                }),
                tlmdb::VariableType::Uint8 => FieldValueSchema::Integral(IntegralFieldSchema {
                    converter,
                    field: GenericIntegralField::U8(build_telemetry_integral_field(bit_range)?),
                }),
                tlmdb::VariableType::Uint16 => FieldValueSchema::Integral(IntegralFieldSchema {
                    converter,
                    field: GenericIntegralField::U16(build_telemetry_integral_field(bit_range)?),
                }),
                tlmdb::VariableType::Uint32 => FieldValueSchema::Integral(IntegralFieldSchema {
                    converter,
                    field: GenericIntegralField::U32(build_telemetry_integral_field(bit_range)?),
                }),
                tlmdb::VariableType::Float => FieldValueSchema::Floating(FloatingFieldSchema {
                    converter: as_polynomial(converter)?,
                    field: GenericFloatingField::F32(build_telemetry_floating_field(bit_range)?),
                }),
                tlmdb::VariableType::Double => FieldValueSchema::Floating(FloatingFieldSchema {
                    converter: as_polynomial(converter)?,
                    field: GenericFloatingField::F64(build_telemetry_floating_field(bit_range)?),
                }),
            },
        },
    ))
}

fn build_telemetry_integral_field<T: Integral>(
    bit_range: Range<usize>,
) -> Result<IntegralField<T>> {
    IntegralField::new(bit_range.clone())
        .ok_or_else(|| anyhow!("invalid bit range: {:?}", bit_range))
}

fn build_telemetry_floating_field<T: Floating>(
    bit_range: Range<usize>,
) -> Result<FloatingField<T>> {
    FloatingField::new(bit_range.clone())
        .ok_or_else(|| anyhow!("invalid bit range: {:?}", bit_range))
}

fn build_integral_converter(conv_info: &tlmdb::ConversionInfo) -> Option<converter::Integral> {
    match conv_info {
        tlmdb::ConversionInfo::None => None,
        tlmdb::ConversionInfo::Hex => None,
        tlmdb::ConversionInfo::Status(status) => {
            Some(converter::Integral::Status(status.clone().into()))
        }
        tlmdb::ConversionInfo::Polynomial(poly) => {
            Some(converter::Integral::Polynomial(poly.clone().into()))
        }
    }
}

fn as_polynomial(converter: Option<converter::Integral>) -> Result<Option<converter::Polynomial>> {
    match converter {
        Some(converter::Integral::Polynomial(poly)) => Ok(Some(poly)),
        Some(converter::Integral::Status(s)) => Err(anyhow!(
            "invalid converter for floating-point number: {:?}",
            s
        )),
        None => Ok(None),
    }
}

fn build_bit_range(extraction_info: &tlmdb::FieldExtractionInfo) -> Option<Range<usize>> {
    let octet_offset = extraction_info.octet_position;
    let bit_offset_local = extraction_info.bit_position;
    let bit_length = extraction_info.bit_length;
    let bit_start_global = bit_offset_local + octet_offset * 8;
    let bit_end_global = bit_start_global + bit_length;
    Some(bit_start_global..bit_end_global)
}

fn iter_fields(
    entries: &[tlmdb::Entry],
) -> impl Iterator<Item = (tlmdb::OnboardSoftwareInfo, &tlmdb::Field)> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            tlmdb::Entry::FieldGroup(group) => {
                Some((&group.onboard_software_info, &group.sub_entries))
            }
            tlmdb::Entry::Comment(_) => None,
        })
        .flat_map(|(obs_info, sub_entries)| {
            sub_entries
                .iter()
                .map(|sub_entry| (obs_info.clone(), sub_entry))
        })
        .filter_map(|(obs_info, sub_entry)| match sub_entry {
            tlmdb::SubEntry::Field(field) => Some((obs_info, field)),
            tlmdb::SubEntry::Comment(_) => None,
        })
}

fn try_integral_to_f64(integral: IntegralValue) -> Result<f64> {
    fn try_i64_to_f64(i: i64) -> Result<f64> {
        let f = i as f64;
        ensure!(f as i64 == i, "failed to cast i64 to f64: {}", i);
        Ok(f)
    }
    fn try_u64_to_f64(u: u64) -> Result<f64> {
        let f = u as f64;
        ensure!(f as u64 == u, "failed to cast u64 to f64: {}", u);
        Ok(f)
    }
    match integral {
        IntegralValue::I8(i) => try_i64_to_f64(i.into()),
        IntegralValue::I16(i) => try_i64_to_f64(i.into()),
        IntegralValue::I32(i) => try_i64_to_f64(i.into()),
        IntegralValue::I64(i) => try_i64_to_f64(i),
        IntegralValue::U8(u) => try_u64_to_f64(u.into()),
        IntegralValue::U16(u) => try_u64_to_f64(u.into()),
        IntegralValue::U32(u) => try_u64_to_f64(u.into()),
        IntegralValue::U64(u) => try_u64_to_f64(u),
    }
}

fn integral_to_bytes(integral: IntegralValue) -> Vec<u8> {
    match integral {
        IntegralValue::I8(i) => i.to_be_bytes().to_vec(),
        IntegralValue::I16(i) => i.to_be_bytes().to_vec(),
        IntegralValue::I32(i) => i.to_be_bytes().to_vec(),
        IntegralValue::I64(i) => i.to_be_bytes().to_vec(),
        IntegralValue::U8(u) => u.to_be_bytes().to_vec(),
        IntegralValue::U16(u) => u.to_be_bytes().to_vec(),
        IntegralValue::U32(u) => u.to_be_bytes().to_vec(),
        IntegralValue::U64(u) => u.to_be_bytes().to_vec(),
    }
}

fn floating_to_bytes(integral: FloatingValue) -> Vec<u8> {
    match integral {
        FloatingValue::F32(f) => f.to_be_bytes().to_vec(),
        FloatingValue::F64(d) => d.to_be_bytes().to_vec(),
    }
}
