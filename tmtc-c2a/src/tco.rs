use std::ops::Deref;

use anyhow::{anyhow, Result};
use gaia_ccsds_c2a::access::cmd::schema::CommandSchema;
use gaia_tmtc::tco_tmiv::{tco_param, Tco};
use structpack::{FloatingValue, IntegralValue};

pub const PARAMETER_NAMES: [&str; 6] = ["param1", "param2", "param3", "param4", "param5", "param6"];

pub struct Reader<'a> {
    tco: &'a Tco,
}

impl<'a> Reader<'a> {
    pub fn new(tco: &'a Tco) -> Self {
        Self { tco }
    }
    pub fn get_value_by_name(&self, name: &str) -> Option<&'a tco_param::Value> {
        self.tco
            .params
            .iter()
            .find(|param| param.name == name)
            .and_then(|param| param.value.as_ref())
    }

    pub fn get_reader_by_name(&self, name: &str) -> Option<ValueReader<'a>> {
        self.get_value_by_name(name)
            .map(|value| ValueReader { value })
    }

    pub fn time_indicator(&self) -> Result<u32> {
        let value = self
            .get_reader_by_name("time_indicator")
            .ok_or_else(|| anyhow!("no time_indicator"))?;
        let integer = value.read_integer()?;
        Ok(integer as u32)
    }

    pub fn parameters(&self) -> Vec<&'a tco_param::Value> {
        let mut values = Vec::with_capacity(PARAMETER_NAMES.len());
        for name in PARAMETER_NAMES.iter() {
            let Some(value) = self.get_value_by_name(name) else {
                break;
            };
            values.push(value);
        }
        values
    }
}

pub struct ValueReader<'a> {
    value: &'a tco_param::Value,
}

impl<'a> ValueReader<'a> {
    pub fn read_integer(&self) -> Result<i64> {
        if let tco_param::Value::Integer(integer) = self.value {
            Ok(*integer)
        } else {
            Err(anyhow!("unexpected data type"))
        }
    }

    #[allow(unused)]
    pub fn read_double(&self) -> Result<f64> {
        if let tco_param::Value::Double(double) = self.value {
            Ok(*double)
        } else {
            Err(anyhow!("unexpected data type"))
        }
    }

    #[allow(unused)]
    pub fn read_bytes(&self) -> Result<&[u8]> {
        if let tco_param::Value::Bytes(bytes) = self.value {
            Ok(bytes)
        } else {
            Err(anyhow!("unexpected data type"))
        }
    }
}

pub struct ParameterListWriter<'a> {
    command_schema: &'a CommandSchema,
}

impl<'a> ParameterListWriter<'a> {
    pub fn new(command_schema: &'a CommandSchema) -> Self {
        Self { command_schema }
    }
}

impl<'a> ParameterListWriter<'a> {
    pub fn write_all<P>(&self, bytes: &mut [u8], parameters: P) -> Result<usize>
    where
        P: Iterator,
        P::Item: Deref<Target = tco_param::Value>,
    {
        let mut writer = self.command_schema.build_writer(bytes);
        for parameter in parameters {
            match parameter.deref() {
                tco_param::Value::Integer(i) => {
                    writer.write(IntegralValue::from(*i).into())?;
                }
                tco_param::Value::Double(d) => {
                    writer.write(FloatingValue::from(*d).into())?;
                }
                tco_param::Value::Bytes(b) => {
                    return writer.write_trailer_and_finish(b);
                }
            }
        }
        writer.finish()
    }
}
