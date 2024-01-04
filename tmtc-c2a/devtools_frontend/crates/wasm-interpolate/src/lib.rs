mod utils;

use interpolator::{format, Formattable};
use wasm_bindgen::prelude::*;

use anyhow::{anyhow, Result};
use js_sys::BigInt;
use std::collections::HashMap;

enum Value {
    I64(i64),
    F64(f64),
    String(String),
}

impl Value {
    pub fn formattable(&self) -> Formattable {
        use Value::*;
        match self {
            I64(v) => Formattable::integer(v),
            F64(v) => Formattable::float(v),
            String(v) => Formattable::display(v),
        }
    }
}

impl TryFrom<&JsValue> for Value {
    type Error = anyhow::Error;
    fn try_from(value: &JsValue) -> Result<Self> {
        if value.is_bigint() {
            let value = BigInt::new(&value)
                .map_err(|_| anyhow!("not a bigint"))?
                .try_into()
                .map_err(|_| anyhow!("couldn't convert bigint to i64"))?;
            Ok(Value::I64(value))
        } else if let Some(v) = value.as_f64() {
            Ok(Value::F64(v))
        } else if let Some(s) = value.as_string() {
            Ok(Value::String(s))
        } else {
            Err(anyhow!("not a string, f64, or bigint"))
        }
    }
}

pub fn format_value_inner(format_string: &str, arg: &JsValue) -> Result<String> {
    let arg = Value::try_from(arg)?;
    let arg = arg.formattable();
    let args = HashMap::from([("value", arg)]);
    format(format_string, &args).map_err(Into::into)
}

#[wasm_bindgen]
pub fn format_value(format_string: &str, arg: &JsValue) -> Result<String, String> {
    format_value_inner(format_string, arg).map_err(|e| e.to_string())
}
