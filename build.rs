extern crate cbindgen;

use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let crate_dir = env::var("CARGO_MANIFEST_DIR")?;

    cbindgen::generate(crate_dir)?.write_to_file("mmatamm_interface.h");

    Ok(())
}
