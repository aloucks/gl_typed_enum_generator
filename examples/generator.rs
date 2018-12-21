extern crate gl_generator;
extern crate gl_typed_enum_generator;

use gl_typed_enum_generator::StructGenerator;
use gl_generator::{Api, Fallbacks, Profile, Registry};

fn main() -> Result<(), Box<std::error::Error>> {
    let registry = Registry::new(Api::Gl, (4, 5), Profile::Core, Fallbacks::All, []);
    let out = std::io::stdout();
    let mut out = out.lock();
    registry.write_bindings(StructGenerator, &mut out)?;
    Ok(())
}