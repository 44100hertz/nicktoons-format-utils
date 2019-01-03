use std::{
    io::{Write},
    fs::{self, File},
};

fn main() {
    let mapstring = fs::read_to_string("jsonmaps/dannyphantomlevel1.json").unwrap();
    let out = trb::dump_file(&mapstring);
    let mut outfile = File::create("testout.trb").unwrap();
    outfile.write(out.as_slice());
}

mod trb {
    use serde_derive::{Deserialize};
    use std::collections::HashMap;


    pub fn dump_file(string: &str) -> Vec<u8> {
        let json: Value = serde_json::from_str(string).unwrap();
        let mut out = vec![];
        if let Value::List(list) = json {
            let mut head = list.len() * 20 + 8;
            for entity in &list {
                out.append(&mut entity.dump_head(&mut head));
            }
            out.append(&mut dump_int(0));
            out.append(&mut dump_int(list.len() as i32));
            for entity in &list {
                out.append(&mut entity.dump_entity(&mut head, out.len()));
            }
        }
        out
    }

    #[derive(Debug, Deserialize)]
    pub enum Value {
        Bool(bool),
        Integer(i32),
        Floating(f32),
        String(String),
        Ident(String), // Usually an aligned, inline String?
        List(Vec<Value>),
        Entity(Entity),
    }

    #[derive(Debug, Deserialize)]
    pub struct Entity {
        Type: String,
        Position: [f32; 4],
        Orientation: [f32; 4],
        ExtraInfo: HashMap<String, Value>,
    }

    impl Value {
        fn dump_head(&self, head: &mut usize) -> Vec<u8> {
            match self {
                Value::Entity(entity) => {
                    // Allocation order
                    let exhead_loc = *head;
                    // Each ExtraInfo header field is a 20 byte struct
                    let exinfo_fields = entity.ExtraInfo.len();
                    *head += exinfo_fields * 20;
                    let type_loc = *head;
                    // Length of nulled, aligned string
                    *head += entity.Type.len();
                    // Align head to next 0 in LSByte...I guess.
                    *head += 0x10 - (*head & 0xf);
                    let matrix_loc = *head;
                    // Length of orient matrix
                    *head += 16*4;
                    let pos_loc = *head;
                    // Length of position
                    *head += 4*4;

                    // Definition order
                    let mut out = vec![];
                    out.append(&mut dump_int(type_loc as i32));
                    out.append(&mut dump_int(exinfo_fields as i32));
                    out.append(&mut dump_int(exhead_loc as i32));
                    out.append(&mut dump_int(matrix_loc as i32));
                    out.append(&mut dump_int(pos_loc as i32));
                    out
                },
                _ => vec![],
            }
        }
        // Used for dumping values inside entity
        fn dump_entity(&self, head: &mut usize, binsize: usize) -> Vec<u8> {
            let mut out = vec![];
            match self {
                Value::Entity(entity) => {
                    for (key, value) in &entity.ExtraInfo {
                        out.append(&mut value.dump_exinfo_head(key, head));
                    }
                    let addr = binsize + out.len();
                    println!("{:x}", addr);
                    out.append(&mut dump_aligned_str(&entity.Type, addr));
                    out.append(&mut quaternion_to_matrix(entity.Orientation)
                               .iter().cloned().flat_map(dump_float).collect());
                    out.append(&mut entity.Position
                               .iter().cloned().flat_map(dump_float).collect());
                }
                _ => panic!("Attempt to dump some other value as entity"),
            }
            out
        }
        // Generate ExtraInfo header struct (they come in groups, see dump_entity)
        fn dump_exinfo_head(&self, key: &str, head: &mut usize) -> Vec<u8> {
            let key_pos = *head;
            *head += key.len();
            let mut value = match self {
                Value::Integer(i) => dump_int(*i),
                Value::Bool(b) => dump_int(if *b {1} else {0}),
                Value::Floating(i) => dump_float(*i),
                Value::Ident(_) | Value::String(_) |
                    Value::List(_) | Value::Entity(_) => dump_int(*head as i32),
            };
            let typeid = match self {
                Value::Integer(_) => 0,
                Value::Floating(_) => 4,
                Value::Bool(_) => 5,
                Value::String(_) => 6,
                Value::List(l) => match l.get(0) {
                    Some(Value::Floating(_)) | None => 7, // TODO: lists...
                    Some(Value::Entity(_)) => 8,
                    _ => 255,
                },
                _ => panic!("Unknown typeid"),
            };
            let mut out = vec![];
            out.append(&mut dump_int(key_pos as i32));
            out.append(&mut dump_int(key.len() as i32));
            out.append(&mut dump_int(typeid));
            out.append(&mut dump_int(0));
            out.append(&mut value);
            out
        }
    }

    fn dump_int(int: i32) -> Vec<u8> {
        let int = int as u32;
        (0..4).map(|i| (int >> (24 - i*8)) as u8).collect()
    }

    fn dump_float(float: f32) -> Vec<u8> {
        unsafe {
            let int: i32 = std::mem::transmute(float);
            dump_int(int)
        }
    }

    fn dump_null_str(s: &str) -> Vec<u8> {
        let mut bytes: Vec<_> = s.bytes().collect();
        bytes.push(0);
        bytes
    }

    fn dump_aligned_str(s: &str, mut head: usize) -> Vec<u8> {
        let mut bytes = dump_null_str(s);
        head += bytes.len();
        while head & 0xf != 0 {
            bytes.push(0);
            head += 1;
        }
        bytes
    }

    fn quaternion_to_matrix(quat: [f32; 4]) -> [f32; 16] {
        let (x, y, z, w) = (quat[0], quat[1], quat[2], quat[3]);
        [
            1.0 - 2.0*(y*y + z*z), 2.0*(x*y + z*w), 2.0*(x*z - y*w), 0.0,
            2.0*(x*y - z*w), 1.0 - 2.0*(x*x + z*z), 2.0*(y*z + x*w), 0.0,
            2.0*(x*z + y*w), 2.0*(y*z - x*w), 1.0 - 2.0*(x*x + y*y), 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    }
}
