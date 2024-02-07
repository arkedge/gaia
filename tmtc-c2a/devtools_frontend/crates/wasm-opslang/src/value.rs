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
        self.cast()
    }

    pub fn double(&self) -> Result<f64> {
        self.cast()
    }

    pub fn bool(&self) -> Result<bool> {
        self.cast()
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
        self.cast()
    }

    pub fn datetime(&self) -> Result<chrono::DateTime<chrono::Utc>> {
        self.cast()
    }
}

pub trait Castable {
    const TYPE_NAME: &'static str;
    fn from_value(v: &Value) -> Option<Self>
    where
        Self: Sized;
}

impl Value {
    pub fn cast<T: Castable>(&self) -> Result<T> {
        match T::from_value(self) {
            Some(x) => Ok(x),
            None => type_err(T::TYPE_NAME, self),
        }
    }
}

macro_rules! impl_castable {
    ($t:ty, $tyname:expr, $variant:ident) => {
        impl Castable for $t {
            const TYPE_NAME: &'static str = stringify!($tyname);
            fn from_value(v: &Value) -> Option<Self> {
                match v {
                    Value::$variant(x) => Some(*x as $t),
                    _ => None,
                }
            }
        }
    };
}

impl_castable!(i64, "integer", Integer);
impl_castable!(f64, "double", Double);
impl_castable!(bool, "bool", Bool);
impl_castable!(chrono::Duration, "duration", Duration);
impl_castable!(chrono::DateTime<chrono::Utc>, "datetime", DateTime);
