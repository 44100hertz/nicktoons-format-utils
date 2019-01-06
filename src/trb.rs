/// Interfaces with the allocator in order to make a binary data structure
/// out of a specially-formatted json file.
// TODO: generate second footer BMVS
// TODO: figure out first footer CLER
// TODO: fix rounding errors with orientation calculation

use serde_derive::{Deserialize};
use crate::allocator::{Object};

// Builtin type for trb format, and my json
#[derive(Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Bool(bool),
    Integer(i32),
    Floating(f32),
    String(String),
    List(Vec<Value>),
    EntityList(Vec<Entity>),
}
// Game entity metadata
#[derive(Deserialize)]
pub struct Entity {
    #[serde(rename = "Type")]
    typename: String,
    #[serde(rename = "Position")]
    position: [f32; 4],
    #[serde(rename = "Orientation")]
    orientation: [f64; 4], // <- converted to 4x4 matrix later
    #[serde(rename = "ExtraInfo")]
    extra_info: Vec<ExtraInfoEntry>,
}
// Json for each is {key, type, value}, but type is fed to Value
#[derive(Deserialize)]
pub struct ExtraInfoEntry {
    key: String,
    #[serde(flatten)]
    value: Value,
}

impl Value {
    /// Construct a value tree from a json string
    pub fn from_string(string: &str) -> Self {
        serde_json::from_str(string).unwrap()
    }
    /// Construct and dump a binary data structure.
    pub fn dump(&self) -> Vec<u8> {
        let body = match self {
            Value::EntityList(list) => {
                Object::list(
                    4, vec![
                    self.object(),
                    Object::integer(0),
                    Object::integer(list.len() as u32),
                ])
            }
            _ => panic!("Expected Entities at file root."),
        };
        let body = body.dump();
        let head = {
            Object::list(
                1, vec![
                Object::string("TSFB", false),
                // Filesize
                Object::integer(0x6b6f0),
                //Object::ptr(Object::empty()),
                Object::string("FBRTXRDH", false),
                // I do not know what these are for (yet?)
                Object::integer(0x18), // Length of remainder of header
                Object::integer(0x0001_0001),
                Object::integer(0x1), // Number of remaining bodies
                Object::integer(0x0),
                Object::integer(body.len() as u32),
                Object::integer(0x0),
                Object::integer(0x0),
                Object::string("TCES", false),
                Object::integer(body.len() as u32),
                Object::Bytes(1, body),
                //                Object::Bytes(4, foot),
                ])
        };
        head.dump()
    }
    // Construct a value's most direct representation
    // i.e. after whatever pointers/metadata that refer to it
    fn object(&self) -> Object {
        match self {
            &Value::Integer(i) => Object::integer(i as u32),
            &Value::Floating(f) => Object::float(f),
            &Value::Bool(b) => Object::integer(if b {0x0100_0000} else {0}),
            Value::String(s) => Object::string(&s, true),
            Value::List(list) => {
                Object::list (
                    0x20,
                    list.iter().map(|it| it.object()).collect())
            }
            Value::EntityList(list) => {
                Object::list(
                    0x4,
                    list.iter().map(|ent| ent.object()).collect())
            }
        }
    }
    fn extra_info_typeid(&self) -> u32 {
        // Are there other types? 1, 2, 3, 9+ are not here
        match self {
            Value::Integer(_) =>    0,
            Value::Floating(_) =>   4,
            Value::Bool(_) =>       5,
            Value::String(_) =>     6,
            // I think lists are internally typed by key string
            Value::List(_) =>       7,
            Value::EntityList(_) => 8,
        }
    }
    fn extra_info_list_len(&self) -> usize {
        match self {
            Value::List(l) => l.len(),
            _ => 0,
        }
    }
    // After size/type info, give a 32-bit value or pointer for
    // the extrainfo struct.
    fn extra_info_object(&self) -> Object {
        match self {
            // Size of a List is in inside ExtraInfo header
            Value::List(list) => {
                Object::ptr(Object::list(
                        0x20,
                        list.iter().map(|x| x.extra_info_object()).collect()))
            }
            Value::EntityList(list) => {
                Object::ptr(Object::list(
                        0x4, vec![
                        Object::ptr(self.object()),
                        Object::integer(list.len() as u32)]))
            }
            Value::String(_) => Object::ptr(self.object()),
            _ => self.object(),
        }
    }
}

impl Entity {
    // 5-dword entity header struct
    fn object(&self) -> Object {
        let position = self.position.iter().cloned()
            .map(Object::float).collect();
        let matrix = quaternion_to_matrix(self.orientation).iter().cloned()
            .map(Object::float).collect();
        let extra_info = self.extra_info.iter()
            .map(|info| info.object()).collect();
        Object::Struct(
            0x4, vec![
            (1, Object::integer(self.extra_info.len() as u32)),
            (2, Object::ptr(Object::list(0x4, extra_info))),
            (0, Object::ptr(Object::string(&self.typename, true))),
            (3, Object::ptr(Object::list(0x10, matrix))),
            (4, Object::ptr(Object::list(0x10, position))),
        ])
    }
}

impl ExtraInfoEntry {
    // Gives 5-dword extrainfo header struct
    // See Value::extra_info_... for details
    fn object(&self) -> Object {
        Object::list(0x4, vec![
                     Object::ptr(Object::string(&self.key, true)),
                     Object::integer(self.key.len() as u32),
                     Object::integer(self.value.extra_info_typeid()),
                     Object::integer(self.value.extra_info_list_len() as u32),
                     self.value.extra_info_object(),
        ])
    }
}

// Since I can't seem to get the same results as their math,
// I opted to use more accurate math.
fn quaternion_to_matrix(quat: [f64; 4]) -> Vec<f32> {
    let (x, y, z, w) = (quat[0], quat[1], quat[2], quat[3]);
    let madd = |x: f64| x.mul_add(-2.0, 1.0); // 1.0 - 2.0 * x
    [
        madd(y*y + z*z), 2.0*(x*y + z*w), 2.0*(x*z - y*w), 0.0,
        2.0*(x*y - z*w), madd(x*x + z*z), 2.0*(y*z + x*w), 0.0,
        2.0*(x*z + y*w), 2.0*(y*z - x*w), madd(x*x + y*y), 0.0,
        0.0, 0.0, 0.0, 1.0
    ].iter().map(|x| *x as f32).collect()
}
