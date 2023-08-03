pub mod converter;
pub mod schema;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum FieldValue {
    Double(f64),
    Integer(i64),
    Constant(String),
    Bytes(Vec<u8>),
}
