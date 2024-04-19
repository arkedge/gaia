use gaia_stub::tco_tmiv::{tmiv_field, Tmiv, TmivField};
use serde::{Deserialize, Serialize};

pub use gaia_stub::tco_tmiv::tmiv::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Variant {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DataType {
    INTEGER,
    DOUBLE,
    STRING,
    ENUM,
    #[serde(alias = "BASE64")]
    BYTES,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Schema {
    pub name: String,
    pub fields: Vec<FieldSchema>,
}

impl Schema {
    fn validate(&self, normalized_tmiv: &Tmiv) -> Result<(), String> {
        if self.fields.len() != normalized_tmiv.fields.len() {
            return Err(format!(
                "Mismatched the number of fields: expected: {}, actual: {}",
                self.fields.len(),
                normalized_tmiv.fields.len()
            ));
        }
        for (idx, (field_schema, value)) in self
            .fields
            .iter()
            .zip(normalized_tmiv.fields.iter())
            .enumerate()
        {
            field_schema
                .validate(value)
                .map_err(|e| format!("params[{idx}]: {e}"))?;
        }
        Ok(())
    }

    fn normalize(&mut self) {
        self.fields.sort_by(|a, b| a.name.cmp(&b.name));
        for field in &mut self.fields {
            field.normalize();
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FieldSchema {
    pub name: String,
    pub data_type: DataType,
    pub variants: Vec<Variant>,
}

impl FieldSchema {
    fn validate(&self, TmivField { name, value }: &TmivField) -> Result<(), String> {
        self.validate_name(name)?;
        let value = value.as_ref().ok_or("no value")?;
        self.validate_value(value)
            .map_err(|e| format!("{name} is {e}"))?;
        Ok(())
    }

    fn validate_name(&self, name: &str) -> Result<(), String> {
        if self.name != *name {
            return Err(format!(
                "Mismatched param name: expected: {}, actual: {}",
                &self.name, name
            ));
        }
        Ok(())
    }

    fn validate_value(&self, value: &tmiv_field::Value) -> Result<(), String> {
        match (&self.data_type, value) {
            (DataType::INTEGER, tmiv_field::Value::Integer(_))
            | (DataType::DOUBLE, tmiv_field::Value::Double(_))
            | (DataType::STRING, tmiv_field::Value::String(_))
            | (DataType::BYTES, tmiv_field::Value::Bytes(_)) => Ok(()),
            (DataType::ENUM, tmiv_field::Value::Enum(variant)) => {
                self.validate_enum(variant)?;
                Ok(())
            }
            _ => Err("mismatched type".to_string()),
        }
    }

    fn validate_enum(&self, value: &str) -> Result<(), String> {
        self.variants
            .iter()
            .any(|v| v.name == value)
            .then_some(())
            .ok_or_else(|| format!("not a valid variant: {}", value))
    }

    fn normalize(&mut self) {
        self.variants.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

#[derive(Debug, Clone)]
pub struct SchemaSet {
    schemata: Vec<Schema>,
}

impl SchemaSet {
    pub fn new(mut schemata: Vec<Schema>) -> Self {
        schemata.sort_by(|a, b| a.name.cmp(&b.name));
        for schema in &mut schemata {
            schema.normalize();
        }
        Self::new_unchecked(schemata)
    }

    pub fn new_unchecked(schemata: Vec<Schema>) -> Self {
        Self { schemata }
    }

    pub fn sanitize(&self, tmiv: &Tmiv) -> Result<Tmiv, String> {
        let mut normalized_tmiv = tmiv.clone();
        normalize_tmiv(&mut normalized_tmiv);
        self.validate(&normalized_tmiv)?;
        Ok(normalized_tmiv)
    }

    pub fn validate(&self, normalized_tmiv: &Tmiv) -> Result<(), String> {
        let schema = self
            .find_schema_by_name(&normalized_tmiv.name)
            .ok_or_else(|| format!("No matched schema for telemetry {}", &normalized_tmiv.name))?;
        schema.validate(normalized_tmiv)?;
        Ok(())
    }

    pub fn find_schema_by_name(&self, field_name: &str) -> Option<&Schema> {
        self.schemata
            .iter()
            .find(|schema| field_name == schema.name)
    }
}

fn normalize_tmiv(tmiv: &mut Tmiv) {
    tmiv.fields.sort_by(|a, b| a.name.cmp(&b.name))
}
