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

    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", content = "value")]
    pub enum Value {
        Bool(bool),
        Integer(i32),
        Floating(f32),
        String(String),
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

    pub fn dump_file(string: &str) -> Vec<u8> {
//            let head = &mut (list.len() * 20 + 8); // +8 for two bytes below
        let value: Value = serde_json::from_str(string).unwrap();
        let mut object = value.root_object();
        let mut bin = vec![];
        let mut head = object.size(); // First layer of allocation
        loop {
            object.dump_layer(&mut bin, &mut head);
            if let Some(next) = object.next_layer() {
                object = next;
            } else {
                break;
            }
        }
        bin
    }

//    pub fn dump_entities(list: &Vec<Entity>, head: &mut usize, mut out: &mut Vec<u8>) {
        // layer 1: entity headers
        //for entity in list {
         //   out.append(&mut entity.dump_header(head, out.len()));
        //}
        // Why do they put this here?
        //out.append(&mut dump_int(0));
        //out.append(&mut dump_int(list.len() as u32));
        // layer 2: entity data, extrainfo headers
        //for entity in list {
        //    out.append(&mut entity.dump(head, out.len()));
       // }
        // layer 3: extrainfo data
        //for entity in list {
         //   for info in &entity.ExtraInfo {
          //      out.append(&mut info.dump(head, out.len()));
           // }
       // }
        // layer 4: RECURSION (this doesn't work)
//        for entity in list {
//            if let Some(ExtraInfoEntry{value: Value::EntityList(e), ..}) =
//                entity.ExtraInfo.last()
//            {
//                dump_entities(e, head, &mut out);
//            }
//        }
//    }



    impl Entity {
        fn object(&self) -> Object {
            Object::List(0x4, vec![
                Object::Dword(self.ExtraInfo.len() as u32),
                reference(Object::List(0x4,
                            self.ExtraInfo.iter()
                                .map(|info| info.object()).collect())),
                reference(Object::Zstring(self.Type.clone())),
                reference(Object::List(0x10,
                    quaternion_to_matrix(self.Orientation)
                        .iter().cloned().map(create_dword_float).collect()
                )),
                reference(Object::List(0x10, // Unknown actual align value
                        self.Position.iter().cloned()
                            .map(create_dword_float).collect())),
            ])
        }
    }

    impl Value {
        // File root representation of object
        // TODO: Not sure how non-entities trb's do this.
        fn root_object(&self) -> Object {
            match self {
                Value::EntityList(list) => Object::List(4, vec![
                    self.object(),
                    Object::Dword(0),
                    Object::Dword(list.len() as u32),
                ]),
                _ => panic!("Expected Entities at file root."),
            }
        }
        // Value representation of object
        fn object(&self) -> Object {
            match self {
                &Value::Integer(i) => Object::Dword(i as u32),
                &Value::Floating(f) => create_dword_float(f),
                &Value::Bool(b) => Object::Dword(if b {0x0100_0000} else {0}),
                Value::String(s) => Object::Zstring(s.clone()),
                Value::List(list) => Object::List(0x20, list.iter()
                        .map(|it| it.object()).collect()),
                Value::EntityList(list) => Object::List(4,
                    list.iter().map(|it| it.object()).collect()
                ),
            }
        }
        fn exinfo_typeid(&self) -> u32 {
            match self {
                Value::Integer(_) =>    0,
                Value::Floating(_) =>   4,
                Value::Bool(_) =>       5,
                Value::String(_) =>     6,
                Value::List(_) =>       7,
                Value::EntityList(_) => 8,
            }
        }
        fn exinfo_object(&self) -> Object {
            let obj = match self {
                // Size of this list is in inside ExtraInfo header
                // IDK how it knows the type of list entry. Hardcoded by key?
                Value::List(list) => reference(Object::List(0x20, list.iter()
                        .map(|it| it.exinfo_object()).collect())),
                // Size of THIS list is right here
                Value::EntityList(list) => reference(
                        Object::List(0x4, vec![
                            // why did they align this like this
                            Object::List(0x10,
                                vec![reference(self.object())],
                            ),
                            Object::Dword(list.len() as u32),
                        ])
                    ),
                Value::String(_) => reference(self.object()),
                _ => self.object(),
            };
            obj
        }
        fn exinfo_list_len(&self) -> usize {
            match self {
                Value::List(l) => l.len(),
                _ => 0,
            }
        }
    }

    impl ExtraInfoEntry {
        fn object(&self) -> Object {
            Object::List(0x4, vec![ // Alignment unknown
                reference(Object::Zstring(self.key.clone())),
                Object::Dword(self.key.len() as u32),
                Object::Dword(self.value.exinfo_typeid()),
                Object::Dword(self.value.exinfo_list_len() as u32),
                self.value.exinfo_object(),
            ])
        }
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

    // Allocation Object
    // Doesn't care about much but memory layout!
    #[derive(Debug)]
    pub enum Object {
        Dword(u32),
        Reference(Box<Object>),
        List(usize, Vec<Object>), // usize for alignment
        Zstring(String),
    }

    fn create_dword_float(float: f32) -> Object {
        unsafe {
            Object::Dword(std::mem::transmute(float))
        }
    }
    fn reference(obj: Object) -> Object {
        Object::Reference(Box::new(obj))
    }

    fn dump_int(int: u32) -> Vec<u8> {
        (0..4).map(|i| (int >> (24 - i*8)) as u8).collect()
    }
    impl Object {
        fn dump_layer(&self, bin: &mut Vec<u8>, head: &mut usize) {
            bin.resize(self.align(bin.len()), 0);
            match self {
                Object::Reference(obj) => {
                    *head = obj.align(*head);
                    bin.extend(dump_int(*head as u32));
                    *head += obj.size();
                }
                Object::Dword(i) => bin.extend(dump_int(*i)),
                Object::List(_,list) => for l in list { l.dump_layer(bin, head) },
                Object::Zstring(s) => {
                    bin.extend(s.bytes());
                    bin.push(0);
                }
            }
        }
        fn next_layer(self) -> Option<Object> {
            match self {
                Object::Reference(obj) => Some(*obj),
                Object::List(_,list) => {
                    if list.len() > 0 {
                        Some(Object::List(1,
                                list.into_iter().filter_map(|item| item.next_layer()).collect()))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        fn align(&self, num: usize) -> usize {
            let align = self.alignment();
            let rem = num % align;
            if rem == 0 { num } else { num + align - rem }
        }
        fn alignment(&self) -> usize {
            match self {
                Object::Reference(_) | Object::Dword(_) => 4,
                &Object::List(align,_) => align,
                Object::Zstring(_) => 1,
            }
        }
        fn size(&self) -> usize {
            match self {
                Object::Reference(_) | Object::Dword(_) => 4,
                Object::List(_,list) => list.iter().fold(0, |acc, x| acc + x.size()),
                Object::Zstring(s) => s.len() + 1,
            }
        }
    }

}
