use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Status {
    map: HashMap<i64, String>,
    default_label: String,
}

impl Status {
    pub fn new(map: HashMap<i64, String>, default_label: String) -> Self {
        Self { map, default_label }
    }

    pub fn convert(&self, value: i64) -> String {
        self.map.get(&value).unwrap_or(&self.default_label).clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial {
    a: [f64; 6],
}

impl Polynomial {
    pub fn new(a: [f64; 6]) -> Self {
        Self { a }
    }
}

impl Polynomial {
    pub fn convert(&self, x: f64) -> f64 {
        self.a
            .iter()
            .enumerate()
            .fold(0f64, |acc, (i, a)| acc + (a * x.powi(i as i32)))
    }
}

#[derive(Debug, Clone)]
pub enum Integral {
    Status(Status),
    Polynomial(Polynomial),
}

impl From<tlmcmddb::tlm::conversion::Status> for Status {
    fn from(db: tlmcmddb::tlm::conversion::Status) -> Self {
        let map = db.variants.into_iter().map(|v| (v.key, v.value)).collect();
        Self::new(map, db.default_value.unwrap_or_else(|| "OTHER".to_string()))
    }
}

impl From<tlmcmddb::tlm::conversion::Polynomial> for Polynomial {
    fn from(
        tlmcmddb::tlm::conversion::Polynomial {
            a0,
            a1,
            a2,
            a3,
            a4,
            a5,
        }: tlmcmddb::tlm::conversion::Polynomial,
    ) -> Self {
        Self::new([a0, a1, a2, a3, a4, a5])
    }
}
