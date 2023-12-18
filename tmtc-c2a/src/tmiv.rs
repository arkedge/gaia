use anyhow::Result;
use gaia_ccsds_c2a::ccsds_c2a::aos::space_packet::SpacePacket;
use gaia_tmtc::tco_tmiv::{tmiv_field, TmivField};

use crate::registry::{StructTelemetrySchema, TelemetrySchema};

pub struct FieldsBuilder<'a> {
    schema: &'a TelemetrySchema,
}

impl<'a> FieldsBuilder<'a> {
    pub fn new(schema: &'a TelemetrySchema) -> Self {
        Self { schema }
    }

    fn build_integral_fields(
        schema: &StructTelemetrySchema,
        fields: &mut Vec<TmivField>,
        bytes: &[u8],
    ) -> Result<()> {
        for (name_pair, field_schema) in schema.integral_fields.iter() {
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

    fn build_floating_fields(
        schema: &StructTelemetrySchema,
        fields: &mut Vec<TmivField>,
        bytes: &[u8],
    ) -> Result<()> {
        for (name_pair, field_schema) in schema.floating_fields.iter() {
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

    fn build_blob_fields(
        fields: &mut Vec<TmivField>,
        space_packet: SpacePacket<&[u8]>,
    ) -> Result<()> {
        fields.push(TmivField {
            name: "@blob".to_string(),
            value: Some(tmiv_field::Value::Bytes(space_packet.user_data.to_vec())),
        });
        fields.push(TmivField {
            name: "@sequence_count".to_string(),
            value: Some(tmiv_field::Value::Integer(
                space_packet.primary_header.sequence_count() as i64,
            )),
        });
        fields.push(TmivField {
            name: "@sequence_flag".to_string(),
            value: Some(tmiv_field::Value::Integer(
                space_packet.primary_header.sequence_flag() as i64,
            )),
        });
        Ok(())
    }

    pub fn build(
        &self,
        tmiv_fields: &mut Vec<TmivField>,
        space_packet_bytes: &[u8],
        space_packet: SpacePacket<&[u8]>,
    ) -> Result<()> {
        match self.schema {
            TelemetrySchema::Struct(struct_schema) => {
                Self::build_integral_fields(struct_schema, tmiv_fields, space_packet_bytes)?;
                Self::build_floating_fields(struct_schema, tmiv_fields, space_packet_bytes)?;
            }
            TelemetrySchema::Blob => {
                Self::build_blob_fields(tmiv_fields, space_packet)?;
            }
        }
        Ok(())
    }
}
