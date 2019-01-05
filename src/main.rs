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
    let mut out_path = PathBuf::from("trb_gen/out");
    for file in fs::read_dir(in_path).unwrap().map(|f| f.unwrap()) {
        out_path.set_file_name(&file.file_name());
        out_path.set_extension("trb");
        println!("{:?} => {:?}", &file.path(), &out_path);
        let mapstring = fs::read_to_string(file.path()).unwrap();
        let out = trb::Value::from_string(&mapstring).dump();
        let mut outfile = File::create(&out_path).unwrap();
        outfile.write(out.as_slice()).unwrap();
    }
}

mod trb {
    use serde_derive::{Deserialize};
    use crate::allocator::{Object};

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
        #[serde(rename = "Type")]
        typename: String,
        #[serde(rename = "Position")]
        position: [f32; 4],
        #[serde(rename = "Orientation")]
        orientation: [f64; 4], // <- converted to 4x4 matrix later
        // I'd use a map, but I like to preserve ordering, and so do they.
        // I'm sure ExtraInfo is read into a map by the game.
        #[serde(rename = "ExtraInfo")]
        extra_info: Vec<ExtraInfoEntry>,
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
                    Object::integer(0),
                    Object::integer(list.len() as u32),
                ]),
                _ => panic!("Expected Entities at file root."),
            };
            let body = body.dump();
            let head = Object::list(1, vec![
                Object::raw_string("TSFB"),
                Object::ptr(Object::empty()), // filesize; empty reference to EOF
                Object::raw_string("FBRTXRDH"),
                // I do not know what these are for (yet?)
                Object::integer(0x18),
                Object::integer(0x00010001),
                Object::integer(0x1),
                Object::integer(0x0),
                Object::integer(body.len() as u32),
                Object::integer(0x0),
                Object::integer(0x0),
                Object::raw_string("TCES"),
                Object::integer(body.len() as u32),
                Object::Bytes(1, body),
            ]);
            head.dump()
        }
        // Value representation of object
        // i.e. after whatever pointers/metadata that refer to it
        fn object(&self) -> Object {
            match self {
                // Primitive values
                &Value::Integer(i) => Object::integer(i as u32),
                &Value::Floating(f) => Object::float(f),
                &Value::Bool(b) => Object::integer(if b {0x0100_0000} else {0}),
                Value::String(s) => Object::zstring(&s),
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
        fn extra_info_object(&self) -> Object {
            match self {
                // Size of this list is in inside ExtraInfo header
                Value::List(list) => Object::ptr(Object::list(0x20, list.iter()
                        .map(|it| it.extra_info_object()).collect())),
                // Size of THIS list is right here
                Value::EntityList(list) => Object::ptr(
                        Object::list(0x4, vec![
                            Object::ptr(self.object()),
                            Object::integer(list.len() as u32),
                        ])
                    ),
                // List entries are always one dword large
                Value::String(_) => Object::ptr(self.object()),
                _ => self.object(),
            }
        }
        fn extra_info_list_len(&self) -> usize {
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
                (1, Object::integer(self.extra_info.len() as u32)),
                (2, Object::ptr(Object::list(0x4,
                            self.extra_info.iter()
                                .map(|info| info.object()).collect()))),
                (0, Object::ptr(Object::zstring(&self.typename))),
                (3, Object::ptr(Object::list(0x10,
                    quaternion_to_matrix(self.orientation)
                        .iter().cloned().map(Object::float).collect()
                ))),
                (4, Object::ptr(Object::list(0x10, // Unknown alignment
                        self.position.iter().cloned()
                            .map(Object::float).collect()))),
            ])
        }
    }

    impl ExtraInfoEntry {
        // Gives 5-dword extrainfo header struct
        // See Value::extra_info_... for details
        fn object(&self) -> Object {
            Object::list(0x4, vec![
                Object::ptr(Object::zstring(&self.key)),
                Object::integer(self.key.len() as u32),
                Object::integer(self.value.extra_info_typeid()),
                Object::integer(self.value.extra_info_list_len() as u32),
                self.value.extra_info_object(),
            ])
        }
    }

    // Since I can't seem to get the same results as their math,
    // I opted to use more accurate math. It is a bit more accurate.
    fn quaternion_to_matrix(quat: [f64; 4]) -> Vec<f32> {
        let (x, y, z, w) = (quat[0], quat[1], quat[2], quat[3]);
        [
            (y*y + z*z).mul_add(-2.0, 1.0), 2.0*(x*y + z*w), 2.0*(x*z - y*w), 0.0,
            2.0*(x*y - z*w), (x*x + z*z).mul_add(-2.0, 1.0), 2.0*(y*z + x*w), 0.0,
            2.0*(x*z + y*w), 2.0*(y*z - x*w), (x*x + y*y).mul_add(-2.0, 1.0), 0.0,
            0.0, 0.0, 0.0, 1.0
        ].iter().map(|x| *x as f32).collect()
    }
}

mod allocator {
    // They use a breadth-first allocator,
    // sort of like going across each layer of a tree in order.

    // A memory object. Each variant fills space differently.
    #[derive(Debug)]
    pub enum Object {
        // A single pointer, and the object it carries
        Reference(Box<Object>),
        // Multiple values. First usize alignment.
        // First usize of vec pair is offset.
        Struct(usize, Vec<(usize, Object)>),
        // Single value. usize is alignment.
        Bytes(usize, Vec<u8>),
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
                Object::Struct(_,list) => {
                    let mut fields = vec![];
                    fields.resize(list.len(), vec![]);
                    let mut binhead = binhead + bin.len();
                    for (pos, obj) in list {
                        fields[*pos] = obj.dump_layer(binhead, head);
                        binhead += fields[*pos].len();
                    }
                    bin.extend(fields.iter().flatten())
                }
                Object::Bytes(_,r) => bin.extend(r),
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
                Object::Bytes(..) => None,
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
                Object::Reference(_) => 4,
                &Object::Struct(align,_) | &Object::Bytes(align,_) => align,
            }
        }
        fn size(&self) -> usize {
            match self {
                Object::Reference(_) => 4,
                Object::Struct(_,list) => {
                    list.iter().fold(0, |acc, (_,x)| acc + x.size())
                }
                Object::Bytes(_,r) => r.len(),
            }
        }

        // Convenience functions/constructors

        pub fn ptr(object: Object) -> Object {
            Object::Reference(Box::new(object))
        }
        // This was once a data type!
        pub fn list(align: usize, object: Vec<Object>) -> Object {
            Object::Struct(align, object.into_iter().enumerate().collect())
        }
        // Big endian dword -> byte dumper
        pub fn integer(int: u32) -> Object {
            Object::Bytes(4, dump_int(int))
        }
        pub fn float(float: f32) -> Object {
            unsafe { Object::integer(std::mem::transmute(float)) }
        }
        pub fn zstring(string: &str) -> Object {
            let mut bytes: Vec<_> = string.bytes().collect();
            bytes.push(0);
            Object::Bytes(1, bytes)
        }
        pub fn raw_string(string: &str) -> Object {
            Object::Bytes(1, string.bytes().collect())
        }
        pub fn empty() -> Object {
            Object::Bytes(1, vec![])
        }
    }
    pub fn dump_int(int: u32) -> Vec<u8> {
        (0..4).map(|i| (int >> (24 - i*8)) as u8).collect()
    }
}

