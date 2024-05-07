use wasm_bindgen::prelude::*;
#[wasm_bindgen(typescript_custom_section)]
const TS_SECTION_VALUE: &str = r#"
type Value =
  {kind : "integer", value : bigint } |
  {kind : "double", value : number} |
  {kind : "bool", value : boolean } |
  {kind: "array", value : Value[] } |
  {kind: "bytes", value : Uint8Array } |
  {kind: "string", value: string } |
  {kind: "duration", value: bigint } |
  {kind: "datetime", value: bigint }
"#;

#[wasm_bindgen(module = "/js/union.js")]
extern "C" {
    #[wasm_bindgen(js_name = "Value", typescript_type = "Value")]
    pub type UnionValue;

    #[wasm_bindgen(js_name = "asInt")]
    fn as_int(_: &UnionValue) -> Option<i64>;
    #[wasm_bindgen(js_name = "asDouble")]
    fn as_double(_: &UnionValue) -> Option<f64>;
    #[wasm_bindgen(js_name = "asBool")]
    fn as_bool(_: &UnionValue) -> Option<bool>;
    #[wasm_bindgen(js_name = "asArray")]
    fn as_array(_: &UnionValue) -> Option<Vec<UnionValue>>;
    #[wasm_bindgen(js_name = "asBytes")]
    fn as_bytes(_: &UnionValue) -> Option<Vec<u8>>;
    #[wasm_bindgen(js_name = "asString")]
    fn as_string(_: &UnionValue) -> Option<std::string::String>;
    #[wasm_bindgen(js_name = "asDuration")]
    fn as_duration(_: &UnionValue) -> Option<i64>;
    #[wasm_bindgen(js_name = "asDateTime")]
    fn as_datetime(_: &UnionValue) -> Option<i64>;

    #[wasm_bindgen(js_name = "makeInt")]
    fn make_int(_: i64) -> UnionValue;
    #[wasm_bindgen(js_name = "makeDouble")]
    fn make_double(_: f64) -> UnionValue;
    #[wasm_bindgen(js_name = "makeBool")]
    fn make_bool(_: bool) -> UnionValue;
    #[wasm_bindgen(js_name = "makeArray")]
    fn make_array(_: Vec<UnionValue>) -> UnionValue;
    #[wasm_bindgen(js_name = "makeBytes")]
    fn make_bytes(_: Vec<u8>) -> UnionValue;
    #[wasm_bindgen(js_name = "makeString")]
    fn make_string(_: std::string::String) -> UnionValue;
    #[wasm_bindgen(js_name = "makeDuration")]
    fn make_duration(_: i64) -> UnionValue;
    #[wasm_bindgen(js_name = "makeDateTime")]
    fn make_datetime(_: i64) -> UnionValue;
}

use crate::Value;
use Value::*;

impl From<UnionValue> for Value {
    fn from(v: UnionValue) -> Value {
        if let Some(v) = as_int(&v) {
            Integer(v)
        } else if let Some(v) = as_double(&v) {
            Double(v)
        } else if let Some(v) = as_bool(&v) {
            Bool(v)
        } else if let Some(vs) = as_array(&v) {
            let vs = vs.into_iter().map(Into::into).collect();
            Array(vs)
        } else if let Some(v) = as_bytes(&v) {
            Bytes(v)
        } else if let Some(v) = as_string(&v) {
            String(v)
        } else if let Some(v) = as_duration(&v) {
            Duration(chrono::Duration::milliseconds(v))
        } else if let Some(v) = as_datetime(&v) {
            DateTime(chrono::DateTime::UNIX_EPOCH + chrono::Duration::milliseconds(v))
        } else {
            unreachable!()
        }
    }
}

impl From<Value> for UnionValue {
    fn from(v: Value) -> UnionValue {
        match v {
            Integer(v) => make_int(v),
            Double(v) => make_double(v),
            Bool(v) => make_bool(v),
            Array(vs) => {
                let vs = vs.into_iter().map(Into::into).collect();
                make_array(vs)
            }
            Bytes(v) => make_bytes(v),
            String(v) => make_string(v),
            Duration(v) => make_duration(v.num_milliseconds()),
            DateTime(v) => make_datetime(v.timestamp_millis()),
        }
    }
}
