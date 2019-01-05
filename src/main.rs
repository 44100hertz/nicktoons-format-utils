// Dump a json made by ini2json into a nicktoons trb.
//
// TODO: fix rounding errors with orientation calculation?
// TODO: look at other trb files
// TODO: figure out meaning of footer

use std::{
    io::{Write},
    fs::{self, File},
    path::{PathBuf},
};

fn main() {
    let in_path = PathBuf::from("jsonmaps");
    let mut out_path = PathBuf::from("trb_gen/out.trb");
    for file in fs::read_dir(in_path).unwrap().map(|f| f.unwrap()) {
        out_path.set_file_name(&file.file_name());
        out_path.set_extension(".trb");
        println!("{:?} => {:?}", &file.path(), &out_path);
        let mapstring = fs::read_to_string(file.path()).unwrap();
        let out = trb::Value::from_string(&mapstring).dump();
        let mut outfile = File::create(&out_path).unwrap();
        outfile.write(out.as_slice());
    }
}

mod trb {
    use serde_derive::{Deserialize};
    use crate::allocator::{Object, reference};

    // Builtin types for trb format, and my json
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
    // All-important game entities
    // Don't let ExtraInfo fool you, all entities have several specific fields.
    #[derive(Debug, Deserialize)]
    pub struct Entity {
        Type: String,
        Position: [f32; 4],
        Orientation: [f32; 4], // <- converted to 4x4 matrix later
        // I'd use a map, but I like to preserve ordering, and so do they.
        // I'm sure ExtraInfo is read into a map by the game.
        ExtraInfo: Vec<ExtraInfoEntry>,
    }
    // Json for each is {key, type, value}, but type is fed to Value
    #[derive(Debug, Deserialize)]
    pub struct ExtraInfoEntry {
        key: String,
        #[serde(flatten)]
        value: Value,
    }

    impl Value {
        pub fn from_string(string: &str) -> Self {
            serde_json::from_str(string).unwrap()
        }
        // File root representation of object
        // TODO: Not sure how non-entities trb's do this.
        pub fn dump(&self) -> Vec<u8> {
            let body = match self {
                Value::EntityList(list) => Object::list(4, vec![
                    self.object(),
                    Object::Dword(0),
                    Object::Dword(list.len() as u32),
                ]),
                _ => panic!("Expected Entities at file root."),
            };
            let body = body.dump();
            let head = Object::list(1, vec![
                Object::Raw("TSFB".bytes().collect()),
                reference(Object::Raw(vec![])), // filesize; empty reference to EOF
                Object::Raw("FBRTXRDH".bytes().collect()),
                // I do not know what these are for (yet?)
                Object::Dword(0x18),
                Object::Dword(0x00010001),
                Object::Dword(0x1),
                Object::Dword(0x0),
                Object::Dword(body.len() as u32),
                Object::Dword(0x0),
                Object::Dword(0x0),
                Object::Raw("TCES".bytes().collect()),
                Object::Dword(body.len() as u32),
                Object::Raw(body),
            ]);
            head.dump()
        }
        // Value representation of object
        // i.e. after whatever pointers/metadata that refer to it
        fn object(&self) -> Object {
            match self {
                // Primitive values
                &Value::Integer(i) => Object::Dword(i as u32),
                &Value::Floating(f) => Object::from_float(f),
                &Value::Bool(b) => Object::Dword(if b {0x0100_0000} else {0}),
                Value::String(s) => Object::Zstring(s.clone()),
                // Plain arrays. Idk why they're aligned so hard.
                Value::List(list) => {
                    Object::list(0x20, list.iter()
                                 .map(|it| it.object()).collect())
                }
                // 5-dword structs of entity metadata.
                // The true "value" of these is under Entity::object
                Value::EntityList(list) => {
                    Object::list(0x4, list.iter()
                                 .map(|ent| ent.object()).collect())
                }
            }
        }
        // Used only by extrainfo, at least right now.
        fn exinfo_typeid(&self) -> u32 {
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
        fn exinfo_object(&self) -> Object {
            match self {
                // Size of this list is in inside ExtraInfo header
                Value::List(list) => reference(Object::list(0x20, list.iter()
                        .map(|it| it.exinfo_object()).collect())),
                // Size of THIS list is right here
                Value::EntityList(list) => reference(
                        Object::list(0x4, vec![
                            reference(self.object()),
                            Object::Dword(list.len() as u32),
                        ])
                    ),
                // List entries are always one dword large
                Value::String(_) => reference(self.object()),
                _ => self.object(),
            }
        }
        fn exinfo_list_len(&self) -> usize {
            match self {
                Value::List(l) => l.len(),
                _ => 0,
            }
        }
    }

    impl Entity {
        // 5-dword entity header struct
        fn object(&self) -> Object {
            Object::Struct(0x4, vec![
                (1, Object::Dword(self.ExtraInfo.len() as u32)),
                (2, reference(Object::list(0x4,
                            self.ExtraInfo.iter()
                                .map(|info| info.object()).collect()))),
                (0, reference(Object::Zstring(self.Type.clone()))),
                (3, reference(Object::list(0x10,
                    quaternion_to_matrix(self.Orientation)
                        .iter().cloned().map(Object::from_float).collect()
                ))),
                (4, reference(Object::list(0x10, // Unknown alignment
                        self.Position.iter().cloned()
                            .map(Object::from_float).collect()))),
            ])
        }
    }

    impl ExtraInfoEntry {
        // Gives 5-dword extrainfo header struct
        // See Value::exinfo_... for details
        fn object(&self) -> Object {
            Object::list(0x4, vec![
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
}

mod allocator {
    // They use a breadth-first allocator,
    // sort of like going across each layer of a tree in order.

    // A memory object. Each variant fills space differently.
    #[derive(Debug)]
    pub enum Object {
        // A 4-byte object; an integer, pointer, float, or bool
        Dword(u32),
        // A single pointer, and the object it carries
        Reference(Box<Object>),
        // Multiple aligned values. usize for alignment.
        // Structs are the only data type with special alignment.
        Struct(usize, Vec<(usize, Object)>),
        // Null-terminated string
        Zstring(String),
        Raw(Vec<u8>),
    }

    impl Object {
        // Allocation loop
        pub fn dump(self) -> Vec<u8> {
            // I have to know where I'm allocating my pointers into,
            // at the same time as writing those pointers.
            //
            // The allocation head starts at the end of the "top layer"
            // of pointers. In the next layer, the head will have reached
            // the layer after that one, etc.
            let mut bin = vec![];
            let mut head = self.size();
            let mut layer = self;
            loop {
                // Collect all the data for this layer
                bin.extend(layer.dump_layer(bin.len(), &mut head));
                // Move to next layer
                if let Some(next) = layer.next_layer() {
                    layer = next;
                } else {
                    break;
                }
            }
            bin
        }

        // Traverse the top layer of an object, and dump it to bytes.
        fn dump_layer(&self, binhead: usize, head: &mut usize) -> Vec<u8> {
            // Big endian dword -> byte dumper
            fn dump_int(int: u32) -> Vec<u8> {
                (0..4).map(|i| (int >> (24 - i*8)) as u8).collect()
            }
            // Align with zeroes
            let mut bin = vec![];
            bin.resize(self.align_amount(binhead), 0);
            match self {
                Object::Reference(obj) => {
                    // Align head
                    *head = obj.align(*head);
                    // Write a pointer to "where it will be"
                    bin.extend(dump_int(*head as u32));
                    // Allocate space
                    *head += obj.size();
                }
                Object::Dword(i) => bin.extend(dump_int(*i)),
                Object::Struct(_,list) => {
                    let mut chunks = vec![];
                    chunks.resize(list.len(), vec![]);
                    let mut binhead = binhead + bin.len();
                    for (pos, obj) in list {
                        chunks[*pos] = obj.dump_layer(binhead, head);
                        binhead += chunks[*pos].len();
                    }
                    bin.extend(chunks.iter().flatten())
                }
                Object::Zstring(s) => {
                    bin.extend(s.bytes());
                    bin.push(0);
                }
                Object::Raw(r) => bin.extend(r),
            }
            bin
        }

        // Cut off the top layer, making the second layer the new top.
        // This is done by following references.
        fn next_layer(self) -> Option<Object> {
            match self {
                Object::Reference(obj) => Some(*obj), // unboxes value
                Object::Struct(_,list) => {
                    let next: Vec<_> = list.into_iter()
                        .filter_map(|(_,obj)| obj.next_layer()).collect();
                    if next.len() > 0 {
                        Some(Object::list(1, next))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        fn align_amount(&self, num: usize) -> usize {
            let align = self.alignment();
            let rem = num % align;
            if rem == 0 { 0 } else { 0 + align - rem }
        }
        // Round up a number to this object's alignment value
        fn align(&self, num: usize) -> usize {
            num + self.align_amount(num)
        }
        fn alignment(&self) -> usize {
            match self {
                Object::Reference(_) | Object::Dword(_) => 4,
                &Object::Struct(align,_) => align,
                Object::Raw(_) | Object::Zstring(_) => 1,
            }
        }
        fn size(&self) -> usize {
            match self {
                Object::Reference(_) | Object::Dword(_) => 4,
                Object::Struct(_,list) => {
                    list.iter().fold(0, |acc, (_,x)| acc + x.size())
                }
                Object::Zstring(s) => s.len() + 1,
                Object::Raw(r) => r.len(),
            }
        }

        // Convenience functions
        pub fn list(align: usize, object: Vec<Object>) -> Object {
            Object::Struct(align, object.into_iter().enumerate().collect())
        }

        // Normally I'd isolate this to Value, but it's such a common thing
        pub fn from_float(float: f32) -> Object {
            unsafe {
                Object::Dword(std::mem::transmute(float))
            }
        }
    }
    // Reference auto-boxer
    pub fn reference(object: Object) -> Object {
        Object::Reference(Box::new(object))
    }
}

