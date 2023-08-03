use anyhow::Result;
use gaia_tmtc::tco_tmiv::{tmiv_field, TmivField};

use crate::registry::TelemetrySchema;

pub struct FieldsBuilder<'a> {
    schema: &'a TelemetrySchema,
}

impl<'a> FieldsBuilder<'a> {
    pub fn new(schema: &'a TelemetrySchema) -> Self {
        Self { schema }
    }

    fn build_integral_fields(&self, fields: &mut Vec<TmivField>, bytes: &[u8]) -> Result<()> {
        for (name_pair, field_schema) in self.schema.integral_fields.iter() {
            let (raw, converted) = field_schema.read_from(bytes)?;
            use gaia_ccsds_c2a::access::tlm::FieldValue;
            let converted = match converted {
                FieldValue::Double(d) => tmiv_field::Value::Double(d),
                FieldValue::Integer(i) => tmiv_field::Value::Integer(i),
                FieldValue::Constant(e) => tmiv_field::Value::Enum(e),
                FieldValue::Bytes(b) => tmiv_field::Value::Bytes(b),
            };
            fields.push(TmivField {
                name: name_pair.raw_name.to_string(),
                value: Some(tmiv_field::Value::Bytes(raw)),
            });
            fields.push(TmivField {
                name: name_pair.converted_name.to_string(),
                value: Some(converted),
            });
        }
        Ok(())
    }

    fn build_floating_fields(&self, fields: &mut Vec<TmivField>, bytes: &[u8]) -> Result<()> {
        for (name_pair, field_schema) in self.schema.floating_fields.iter() {
            let (raw, converted) = field_schema.read_from(bytes)?;
            use gaia_ccsds_c2a::access::tlm::FieldValue;
            let converted = match converted {
                FieldValue::Double(d) => tmiv_field::Value::Double(d),
                FieldValue::Integer(i) => tmiv_field::Value::Integer(i),
                FieldValue::Constant(e) => tmiv_field::Value::Enum(e),
                FieldValue::Bytes(b) => tmiv_field::Value::Bytes(b),
            };
            fields.push(TmivField {
                name: name_pair.raw_name.to_string(),
                value: Some(tmiv_field::Value::Bytes(raw)),
            });
            fields.push(TmivField {
                name: name_pair.converted_name.to_string(),
                value: Some(converted),
            });
        }
        Ok(())
    }

    pub fn build(&self, tmiv_fields: &mut Vec<TmivField>, space_packet_bytes: &[u8]) -> Result<()> {
        self.build_integral_fields(tmiv_fields, space_packet_bytes)?;
        self.build_floating_fields(tmiv_fields, space_packet_bytes)?;
        Ok(())
    }
}
