use crate::Result;
use crate::{type_err, RuntimeError};

#[derive(Debug)]
pub(crate) enum Value {
    Integer(i64),
    Double(f64),
    Bool(bool),
    Array(Vec<Value>),
    String(String),
    Duration(chrono::Duration),
    DateTime(chrono::DateTime<chrono::Utc>),
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Double(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<chrono::Duration> for Value {
    fn from(v: chrono::Duration) -> Self {
        Value::Duration(v)
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Value {
    fn from(v: chrono::DateTime<chrono::Utc>) -> Self {
        Value::DateTime(v)
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        use Value::*;
        match self {
            Integer(_) => "integer",
            Double(_) => "double",
            Bool(_) => "bool",
            Array(_) => "array",
            String(_) => "string",
            Duration(_) => "duration",
            DateTime(_) => "datetime",
        }
    }

    pub fn integer(&self) -> Result<i64> {
        self.try_into()
    }

    pub fn double(&self) -> Result<f64> {
        self.try_into()
    }

    pub fn bool(&self) -> Result<bool> {
        self.try_into()
    }

    pub fn array(&self) -> Result<&Vec<Value>> {
        match self {
            Value::Array(x) => Ok(x),
            _ => type_err("array", self),
        }
    }

    pub fn string(&self) -> Result<&str> {
        match self {
            Value::String(x) => Ok(x),
            _ => type_err("string", self),
        }
    }

    pub fn duration(&self) -> Result<chrono::Duration> {
        self.try_into()
    }

    pub fn datetime(&self) -> Result<chrono::DateTime<chrono::Utc>> {
        self.try_into()
    }
}

impl<'a> TryInto<i64> for &'a Value {
    type Error = RuntimeError;
    fn try_into(self) -> Result<i64> {
        match self {
            Value::Integer(x) => Ok(*x),
            _ => type_err("integer", self),
        }
    }
}

impl<'a> TryInto<f64> for &'a Value {
    type Error = RuntimeError;
    fn try_into(self) -> Result<f64> {
        match self {
            Value::Double(x) => Ok(*x),
            _ => type_err("double", self),
        }
    }
}

impl TryInto<bool> for &Value {
    type Error = RuntimeError;
    fn try_into(self) -> Result<bool> {
        match self {
            Value::Bool(x) => Ok(*x),
            _ => type_err("bool", self),
        }
    }
}

impl TryInto<chrono::Duration> for &Value {
    type Error = RuntimeError;
    fn try_into(self) -> Result<chrono::Duration> {
        match self {
            Value::Duration(x) => Ok(*x),
            _ => type_err("duration", self),
        }
    }
}

impl TryInto<chrono::DateTime<chrono::Utc>> for &Value {
    type Error = RuntimeError;
    fn try_into(self) -> Result<chrono::DateTime<chrono::Utc>> {
        match self {
            Value::DateTime(x) => Ok(*x),
            _ => type_err("duration", self),
        }
    }
}
