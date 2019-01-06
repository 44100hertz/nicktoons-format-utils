mod trb;
mod allocator;

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
        outfile.write_all(out.as_slice()).unwrap();
    }
}


