use std::{fs, io};

fn main() -> Result<(), io::Error> {
    let mut command = capnpc::CompilerCommand::new();
    command.src_prefix("schema");

    for entry in fs::read_dir("schema")? {
        let path = entry?.path();
        if path.extension().is_some_and(|p| p == "capnp") {
            command.file(&path);
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    println!("cargo:rerun-if-changed=schema");
    command.run().expect("schema compiler command");
    Ok(())
}
