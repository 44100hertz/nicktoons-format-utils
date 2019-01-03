// Dump a json made by ini2json into a nicktoons trb.
// Currently trying to replicate their format as closely as I can.
// Size optimizations may be possible; their ini -> trb implementation
// is fishy.

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
    // Redesign idea: "Allocation objects"
    // First, you call 'alloc' with a size, and a callback which gives that-sized data,
    // and is also able to call 'alloc'. I only need to process arrays of said callbacks.
    use serde_derive::{Deserialize};

    pub fn dump_file(string: &str) -> Vec<u8> {
        let json: Value = serde_json::from_str(string).unwrap();
        let out = &mut vec![];
        if let Value::EntityList(list) = json {
            let head = &mut (list.len() * 20 + 8); // +8 for two bytes below
            dump_entities(&list, head, out);
        }
        out.to_vec()
    }

    pub fn dump_entities(list: &Vec<Entity>, head: &mut usize, mut out: &mut Vec<u8>) {
        // layer 1: entity headers
        for entity in list {
            out.append(&mut entity.dump_header(head, out.len()));
        }
        // Why do they put this here?
        out.append(&mut dump_int(0));
        out.append(&mut dump_int(list.len() as u32));
        // layer 2: entity data, extrainfo headers
        for entity in list {
            out.append(&mut entity.dump(head, out.len()));
        }
        // layer 3: extrainfo data
        for entity in list {
            for info in &entity.ExtraInfo {
                out.append(&mut info.dump(head, out.len()));
            }
        }
        // layer 4: RECURSION (this doesn't work)
//        for entity in list {
//            if let Some(ExtraInfoEntry{value: Value::EntityList(e), ..}) =
//                entity.ExtraInfo.last()
//            {
//                dump_entities(e, head, &mut out);
//            }
//        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", content = "value")]
    pub enum Value {
        Bool(bool),
        Integer(i32),
        Floating(f32),
        String(String),
//        Ident(String),
        List(Vec<Value>),
        EntityList(Vec<Entity>),
    }

    #[derive(Debug, Deserialize)]
    pub struct Entity {
        Type: String,
        Position: [f32; 4],
        Orientation: [f32; 4],
        ExtraInfo: Vec<ExtraInfoEntry>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ExtraInfoEntry {
        key: String,
        #[serde(flatten)]
        value: Value,
    }


    impl Entity {
        fn dump_header(&self, head: &mut usize, binsize: usize) -> Vec<u8> {
            // Allocation order
            let exhead_loc = *head;
            // Each ExtraInfo header field is a 20 byte struct
            let exinfo_fields = self.ExtraInfo.len();
            *head += exinfo_fields * 20;
            let type_loc = *head;
            // Nulled, aligned string
            *head += self.Type.len() + 1;
            *head = align(*head);
            let matrix_loc = *head;
            *head += 16*4; // Orientation matrix
            let pos_loc = *head;
            *head += 4*4; // Position

            // Definition order
            let mut out = vec![];
            while binsize % 4 != 0 { out.push(0); }
            out.append(&mut dump_int(type_loc as u32));
            out.append(&mut dump_int(exinfo_fields as u32));
            out.append(&mut dump_int(exhead_loc as u32));
            out.append(&mut dump_int(matrix_loc as u32));
            out.append(&mut dump_int(pos_loc as u32));
            out
        }
        fn dump(&self, head: &mut usize, binsize: usize) -> Vec<u8> {
            let mut out = vec![];
            for info in &self.ExtraInfo {
                out.append(&mut info.value.dump_exinfo_head(&info.key, head));
            }
            let addr = binsize + out.len();
            out.append(&mut dump_str_aligned(&self.Type, addr, 0x10));
            out.append(&mut quaternion_to_matrix(self.Orientation)
                       .iter().cloned().flat_map(dump_float).collect());
            out.append(&mut self.Position
                       .iter().cloned().flat_map(dump_float).collect());
            out
        }
    }

    impl Value {
        // Generate ExtraInfo header struct
        fn dump_exinfo_head(&self, key: &str, head: &mut usize) -> Vec<u8> {
            let key_pos = *head;
            *head += key.len() + 1;
            // All lists, even empty,
            // cause the key string to pad with zeroes for alignment to 0x20.
            match self {
                Value::List(_) | Value::EntityList(_) => *head = roundup(*head, 0x20),
                _ => {},
            }
            // Single byte for header.
            let mut value = self.dump_value(head);
            // Still wondering what other typeids are, if they ever occur.
            let (typeid, list_len) = match self {
                Value::Integer(_) => (0, 0),
                Value::Floating(_) => (4, 0),
                Value::Bool(_) => (5, 0),
                Value::String(_) => (6, 0),
                Value::List(l) => (7, l.len()),
                Value::EntityList(_) => (8, 0),
            };

            let mut out = vec![];
            out.append(&mut dump_int(key_pos as u32));
            out.append(&mut dump_int(key.len() as u32));
            out.append(&mut dump_int(typeid));
            out.append(&mut dump_int(list_len as u32));
            out.append(&mut value);
            out
        }

        fn dump_value(&self, head: &mut usize) -> Vec<u8> {
            let out = match self {
                Value::Integer(i) => dump_int(*i as u32),
                // Guess they use the first byte with bools
                Value::Bool(b) => vec![if *b {1} else {0}, 0, 0, 0],
                Value::Floating(i) => dump_float(*i),
                Value::String(_) | Value::List(_) |
                    Value::EntityList(_) => dump_int(*head as u32),
            };
            let alloc_size = match self {
                Value::Integer(_) | Value::Floating(_) | Value::Bool(_) => 0,
                Value::String(s) => s.len() + 1,
                Value::List(l) => l.len() * 4,
                // EntityList creates a 2-number "EntityList head" later on.
                // With a pointer to the first entity, then the number of ents.
                // That's why it doesn't have a header length.
                Value::EntityList(_) => 8,
            };
            *head += alloc_size;
            out
        }

        fn dump_exinfo_body(&self, head: &mut usize) -> Vec<u8> {
            let out = match self {
                Value::Integer(_) | Value::Floating(_) | Value::Bool(_) => vec![],
                Value::String(s) => dump_str_null(s),
                Value::List(list) => {
                    list.iter().flat_map(|e| e.dump_value(head)).collect()
                }
                Value::EntityList(l) => {
                    *head = roundup(*head, 0x4);
                    let mut ptr = dump_int(*head as u32);
                    ptr.append(&mut dump_int(l.len() as u32));
                    ptr
                }
            };
            let alloc_size = match self {
                Value::EntityList(l) => l.len() * 20,
                _ => 0,
            };
            *head += alloc_size;
            out
        }
    }

    impl ExtraInfoEntry {
        fn dump(&self, head: &mut usize, binsize: usize) -> Vec<u8> {
            let mut out = match self.value {
                Value::List(_) | Value::EntityList(_) =>
                    dump_str_aligned(&self.key, binsize, 0x20),
                _ => dump_str_null(&self.key),
            };
            out.append(&mut self.value.dump_exinfo_body(head));
            out
        }
    }

    fn roundup(num: usize, target: usize) -> usize {
        let rem = num % target;
        if rem == 0 { num } else { num + target - rem }
    }
    // They align to 16 bytes rather than 4 for most strings.
    // For lists, 32 bytes.
    // My theory is that the smallest bits store metadata, i.e.
    // void* real_addr = list_ptr & 0xffff_fff0;
    // enum Type type = list_ptr & 0xf;
    fn align(head: usize) -> usize { roundup(head, 0x10) }

    fn dump_int(int: u32) -> Vec<u8> {
        (0..4).map(|i| (int >> (24 - i*8)) as u8).collect()
    }

    fn dump_float(float: f32) -> Vec<u8> {
        unsafe {
            dump_int(std::mem::transmute(float))
        }
    }

    fn dump_str_null(s: &str) -> Vec<u8> {
        let mut bytes: Vec<_> = s.bytes().collect();
        bytes.push(0);
        bytes
    }

    fn dump_str_aligned(s: &str, mut head: usize, target: usize) -> Vec<u8> {
        let mut bytes = dump_str_null(s);
        head += bytes.len();
        while head % target != 0 {
            bytes.push(0);
            head += 1;
        }
        bytes
    }

    // This calculation still fails to reproduce the rounding errors
    // in their calculations. Their math typically makes smaller numbers.
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
