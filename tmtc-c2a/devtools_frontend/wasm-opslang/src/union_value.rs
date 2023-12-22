use wasm_bindgen::prelude::*;
#[wasm_bindgen(typescript_custom_section)]
const TS_SECTION_VALUE: &str = r#"
type Value =
  {kind : "integer", value : bigint } |
  {kind : "double", value : number} |
  {kind : "bool", value : boolean } |
  {kind: "array", value : Value[] } |
  {kind: "string", value: string }
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
    #[wasm_bindgen(js_name = "asString")]
    fn as_string(_: &UnionValue) -> Option<std::string::String>;

    #[wasm_bindgen(js_name = "makeInt")]
    fn make_int(_: i64) -> UnionValue;
    #[wasm_bindgen(js_name = "makeDouble")]
    fn make_double(_: f64) -> UnionValue;
    #[wasm_bindgen(js_name = "makeBool")]
    fn make_bool(_: bool) -> UnionValue;
    #[wasm_bindgen(js_name = "makeArray")]
    fn make_array(_: Vec<UnionValue>) -> UnionValue;
    #[wasm_bindgen(js_name = "makeString")]
    fn make_string(_: std::string::String) -> UnionValue;
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
        } else if let Some(v) = as_string(&v) {
            String(v)
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
            String(v) => make_string(v),
        }
    }
}

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

//not tested
pub(crate) fn from_jsvalue(v: JsValue) -> Option<Value> {
    use crate::js_sys::Reflect;
    let kind = Reflect::get(&v, &JsValue::from_str("kind"))
        .ok()?
        .as_string()?;
    let value = Reflect::get(&v, &JsValue::from_str("value")).ok()?;
    if kind == "double" {
        value.as_f64().map(Value::Double)
    } else if kind == "bool" {
        value.as_bool().map(Value::Bool)
    } else if kind == "array" {
        todo!("from_jsvalue: array")
    } else if kind == "string" {
        todo!("from_jsvalue: string")
    } else {
        log!("{:?}", value);
        None
    }
}
