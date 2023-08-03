use gaia_stub::tco_tmiv::{tco_param, Tco, TcoParam};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DataType {
    INTEGER,
    DOUBLE,
    BYTES,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Schema {
    pub name: String,
    pub params: Vec<ParamSchema>,
}

impl Schema {
    fn validate(&self, normalized_tco: &Tco) -> Result<(), String> {
        if self.params.len() != normalized_tco.params.len() {
            return Err(format!(
                "Mismatched the number of params: expected: {}, actual: {}",
                self.params.len(),
                normalized_tco.params.len()
            ));
        }
        for (idx, (param_schema, value)) in self
            .params
            .iter()
            .zip(normalized_tco.params.iter())
            .enumerate()
        {
            param_schema
                .validate(value)
                .map_err(|e| format!("params[{idx}]: {e}"))?;
        }
        Ok(())
    }

    fn normalize(&mut self) {
        self.params.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ParamSchema {
    pub name: String,
    pub data_type: DataType,
}

impl ParamSchema {
    fn validate_name(&self, name: &str) -> Result<(), String> {
        if self.name != *name {
            return Err(format!(
                "Mismatched param name: expected: {}, actual: {}",
                &self.name, name
            ));
        }
        Ok(())
    }

    fn validate_value(&self, value: &tco_param::Value) -> Result<(), String> {
        match (&self.data_type, value) {
            (DataType::INTEGER, tco_param::Value::Integer(_))
            | (DataType::DOUBLE, tco_param::Value::Double(_))
            | (DataType::BYTES, tco_param::Value::Bytes(_)) => Ok(()),
            _ => Err("type mismatched".to_string()),
        }
    }

    fn validate(&self, TcoParam { name, value }: &TcoParam) -> Result<(), String> {
        self.validate_name(name)?;
        let value = value.as_ref().ok_or("no value")?;
        self.validate_value(value)
            .map_err(|e| format!("{name} is {e}"))?;
        Ok(())
    }
}

#[derive(Debug)]
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

    pub fn validate(&self, normalized_tco: &Tco) -> Result<(), String> {
        let schema = self
            .find_schema_by_name(&normalized_tco.name)
            .ok_or_else(|| format!("No matched schema for command {}", &normalized_tco.name))?;
        schema.validate(normalized_tco)?;
        Ok(())
    }

    pub fn sanitize(&self, tco: &Tco) -> Result<Tco, String> {
        let mut normalized_tco = tco.clone();
        normalize_tco(&mut normalized_tco);
        self.validate(&normalized_tco)?;
        Ok(normalized_tco)
    }

    fn find_schema_by_name(&self, param_name: &str) -> Option<&Schema> {
        self.schemata
            .iter()
            .find(|schema| param_name == schema.name)
    }
}

fn normalize_tco(tco: &mut Tco) {
    tco.params.sort_by(|a, b| a.name.cmp(&b.name))
}
