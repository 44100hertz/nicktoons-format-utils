/// Currently just a utility to convert specially formatted json files
/// into nicktoons-format trb files, I'm working on making a more mature
/// set of utilities for dealing with their binary format.
///
/// Will dump a directory of json files into jsonmaps/ into trb_gen/.
/// Try ini2json.js in the root of the crate.

pub mod trb;
pub mod allocator;

use std::{
    io::{Write},
    fs::{self, File},
    path::{PathBuf},
};

fn main() -> Result<(), std::io::Error> {
    let in_path = PathBuf::from("jsonmaps/");

    let mut out_path = PathBuf::from("trb_gen/");
    fs::create_dir(&out_path).unwrap_or(());
    out_path.push("out.trb");

    for file in fs::read_dir(in_path)? {
        let file = file?;
        out_path.set_file_name(&file.file_name());
        out_path.set_extension("trb");
        println!("{:?} => {:?}", &file.path(), &out_path);
        let mapstring = fs::read_to_string(file.path())?;
        let out = trb::Value::from_string(&mapstring).dump();
        let mut outfile = File::create(&out_path)?;
        outfile.write_all(out.as_slice())?;
    }
    Ok(())
}


