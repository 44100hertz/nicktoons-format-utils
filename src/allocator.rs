/// A simple allocator capable of making nicktoons binary files.
/// These files use fairly simple allocation, with mostly
/// big-endian 32-bit values.

/// A low-level memory object. Look at the constructors below
/// for easier ways to make them.
pub enum Object {
    // A single pointer, and the object it carries
    Reference(Box<Object>),
    // Multiple values. First usize is alignment.
    // First usize of vec pair is offset.
    Struct(usize, Vec<(usize, Object)>),
    // Single value. usize is alignment.
    Bytes(usize, Vec<u8>),
}

impl Object {
    /// Construct a referenced value. When allocated, will write a pointer,
    /// and add the value inside to the next allocation layer.
    pub fn ptr(object: Object) -> Object {
        Object::Reference(Box::new(object))
    }
    /// Construct a list, or array of objects from a vector.
    /// A list is just a struct of items in order
    pub fn list(align: usize, objects: Vec<Object>) -> Object {
        Object::Struct(align, objects.into_iter().enumerate().collect())
    }
    /// Construct a primitive 32bit integer.
    pub fn integer(int: u32) -> Object {
        Object::Bytes(4, dump_int(int))
    }
    /// Construct a primitive 32bit float.
    pub fn float(float: f32) -> Object {
        unsafe { Object::integer(std::mem::transmute(float)) }
    }
    /// Construct an optionally null-terminated string.
    pub fn string(string: &str, null: bool) -> Object {
        let mut bytes: Vec<_> = string.bytes().collect();
        if null { bytes.push(0); }
        Object::Bytes(1, bytes)
    }

    /// Dump every layer of an allocation object into bytes.
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

    /// Get the actual size of a value
    pub fn size(&self) -> usize {
        match self {
            Object::Reference(_) => 4,
            Object::Struct(_,list) => {
                list.iter().fold(0, |acc, (_,x)| acc + x.size())
            }
            Object::Bytes(_,r) => r.len(),
        }
    }
    /// Get the alignment of a value
    pub fn alignment(&self) -> usize {
        match self {
            Object::Reference(_) => 4,
            &Object::Struct(align,_) | &Object::Bytes(align,_) => align,
        }
    }

    // Traverse the top layer of an object, and dump it to bytes.
    fn dump_layer(&self, binhead: usize, head: &mut usize) -> Vec<u8> {
        // Align with zeroes
        let mut bin = vec![];
        bin.resize(align_amount(binhead, self.alignment()), 0);
        match self {
            Object::Reference(obj) => {
                // Align head
                *head = align(*head, obj.alignment());
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
                if !next.is_empty() {
                    Some(Object::list(1, next))
                } else {
                    None
                }
            }
            Object::Bytes(..) => None,
        }
    }
}

/// Get how many bytes would be needed for alignment
pub fn align_amount(num: usize, align: usize) -> usize {
    let rem = num % align;
    if rem == 0 { 0 } else { align - rem }
}
/// Round up a number for alignment
pub fn align(num: usize, align: usize) -> usize {
    num + align_amount(num, align)
}
/// Makes a big-endian integer
pub fn dump_int(int: u32) -> Vec<u8> {
    (0..4).map(|i| (int >> (24 - i*8)) as u8).collect()
}
