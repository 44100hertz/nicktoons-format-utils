use std::fs;
fn main() {
    let mapstring = fs::read_to_string("jsonmaps/dannyphantomlevel1.json").unwrap();
    let out = trb::values_from_json(&mapstring);
    println!("{:?}", out);
}

mod trb {
    use serde_derive::{Deserialize};
    use std::collections::HashMap;

    pub fn values_from_json(string: &str) -> Value {
        let json: Value = serde_json::from_str(string).unwrap();
        json
    }

    #[derive(Debug, Deserialize)]
    pub enum Value {
        Bool(bool),
        Floating(f32),
        Integer(f32),
        String(String),
        Ident(String),
        List(Vec<Value>),
        Entity(HashMap<String, Value>),
    }

    pub fn quaternion_to_matrix(quat: [f32; 4]) -> [f32; 16] {
        let (x, y, z, w) = (quat[0], quat[1], quat[2], quat[3]);
        [
            1.0 - 2.0*(y*y + z*z), 2.0*(x*y + z*w), 2.0*(x*z - y*w), 0.0,
            2.0*(x*y - z*w), 1.0 - 2.0*(x*x + z*z), 2.0*(y*z + x*w), 0.0,
            2.0*(x*z + y*w), 2.0*(y*z - x*w), 1.0 - 2.0*(x*x + y*y), 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    }
}
