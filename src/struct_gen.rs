// Copyright 2015 Brendan Zabarauskas and the gl-rs developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHdest WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use gl_generator::{Registry, Cmd};
use gl_generator::generators;

use std::io;

#[allow(missing_copy_implementations)]
pub struct StructGenerator;

impl generators::Generator for StructGenerator {
    fn write<W>(&self, registry: &Registry, dest: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        try!(write_header(dest));
        try!(write_type_aliases(registry, dest));
        try!(write_enums(registry, dest));
        try!(write_fnptr_struct_def(dest));
        try!(write_panicking_fns(registry, dest));
        try!(write_struct(registry, dest));
        try!(write_impl(registry, dest));
        try!(write_enum_groups(registry, dest));
        Ok(())
    }
}

/// Creates a `__gl_imports` module which contains all the external symbols that we need for the
///  bindings.
fn write_header<W>(dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    writeln!(
        dest,
        r#"
        mod __gl_imports {{
            pub use std::mem;
            pub use std::marker::Send;
            pub use std::os::raw;
        }}
    "#
    )
}

/// Creates a `types` module which contains all the type aliases.
///
/// See also `generators::gen_types`.
fn write_type_aliases<W>(registry: &Registry, dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    try!(writeln!(
        dest,
        r#"
        pub mod types {{
            #![allow(non_camel_case_types, non_snake_case, dead_code, missing_copy_implementations)]
    "#
    ));

    try!(generators::gen_types(registry.api, dest));

    writeln!(dest, "}}")
}

/// Creates all the `<enum>` elements at the root of the bindings.
fn write_enums<W>(registry: &Registry, dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    for enm in &registry.enums {
        try!(generators::gen_enum_item(enm, "types::", dest));
    }

    Ok(())
}

/// Creates a `FnPtr` structure which contains the store for a single binding.
fn write_fnptr_struct_def<W>(dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    writeln!(
        dest,
        "
        #[allow(dead_code, missing_copy_implementations)]
        #[derive(Clone)]
        pub struct FnPtr {{
            /// The function pointer that will be used when calling the function.
            f: *const __gl_imports::raw::c_void,
            /// True if the pointer points to a real function, false if points to a `panic!` fn.
            is_loaded: bool,
        }}

        impl FnPtr {{
            /// Creates a `FnPtr` from a load attempt.
            fn new(ptr: *const __gl_imports::raw::c_void) -> FnPtr {{
                if ptr.is_null() {{
                    FnPtr {{
                        f: missing_fn_panic as *const __gl_imports::raw::c_void,
                        is_loaded: false
                    }}
                }} else {{
                    FnPtr {{ f: ptr, is_loaded: true }}
                }}
            }}

            /// Returns `true` if the function has been successfully loaded.
            ///
            /// If it returns `false`, calling the corresponding function will fail.
            #[inline]
            #[allow(dead_code)]
            pub fn is_loaded(&self) -> bool {{
                self.is_loaded
            }}
        }}
    "
    )
}

/// Creates a `panicking` module which contains one function per GL command.
///
/// These functions are the mocks that are called if the real function could not be loaded.
fn write_panicking_fns<W>(registry: &Registry, dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    writeln!(
        dest,
        "#[inline(never)]
        fn missing_fn_panic() -> ! {{
            panic!(\"{api} function was not loaded\")
        }}",
        api = registry.api
    )
}

/// Creates a structure which stores all the `FnPtr` of the bindings.
///
/// The name of the struct corresponds to the namespace.
fn write_struct<W>(registry: &Registry, dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    try!(writeln!(
        dest,
        "
        #[allow(non_camel_case_types, non_snake_case, dead_code)]
        #[derive(Clone)]
        pub struct {api}FnPtrs {{",
        api = generators::gen_struct_name(registry.api)
    ));

    for cmd in &registry.cmds {
        if let Some(v) = registry.aliases.get(&cmd.proto.ident) {
            try!(writeln!(dest, "/// Fallbacks: {}", v.join(", ")));
        }
        try!(writeln!(dest, "pub {name}: FnPtr,", name = cmd.proto.ident));
    }
    //try!(writeln!(dest, "_priv: ()"));

    writeln!(dest, "}}")?;

    writeln!(dest, "")?;

    writeln!(
        dest,
        "
        #[allow(non_camel_case_types, non_snake_case, dead_code)]
        #[derive(Clone)]
        pub struct {api} {{
            pub ptrs: {api}FnPtrs,
            _priv: (),
        }}
        ",
        api = generators::gen_struct_name(registry.api)
    )?;

    Ok(())
}

/// Creates the `impl` of the structure created by `write_struct`.
fn write_impl<W>(registry: &Registry, dest: &mut W) -> io::Result<()>
where
    W: io::Write,
{
    try!(writeln!(dest,
                  "impl {api} {{
            /// Load each OpenGL symbol using a custom load function. This allows for the
            /// use of functions like `glfwGetProcAddress` or `SDL_GL_GetProcAddress`.
            ///
            /// ~~~ignore
            /// let gl = Gl::load_with(|s| glfw.get_proc_address(s));
            /// ~~~
            #[allow(dead_code, unused_variables)]
            pub fn load_with<F>(mut loadfn: F) -> {api} where F: FnMut(&'static str) -> *const __gl_imports::raw::c_void {{
                #[inline(never)]
                fn do_metaloadfn(loadfn: &mut FnMut(&'static str) -> *const __gl_imports::raw::c_void,
                                 symbol: &'static str,
                                 symbols: &[&'static str])
                                 -> *const __gl_imports::raw::c_void {{
                    let mut ptr = loadfn(symbol);
                    if ptr.is_null() {{
                        for &sym in symbols {{
                            ptr = loadfn(sym);
                            if !ptr.is_null() {{ break; }}
                        }}
                    }}
                    ptr
                }}
                let mut metaloadfn = |symbol: &'static str, symbols: &[&'static str]| {{
                    do_metaloadfn(&mut loadfn, symbol, symbols)
                }};
                {api}::load_with_metaloadfn(&mut metaloadfn)
            }}

            #[inline(never)]
            fn load_with_metaloadfn(metaloadfn: &mut FnMut(&'static str, &[&'static str]) -> *const __gl_imports::raw::c_void) -> {api} {{
                
                {api} {{
                    ptrs: {api}FnPtrs {{",
                  api = generators::gen_struct_name(registry.api)));

    for cmd in &registry.cmds {
        try!(writeln!(
            dest,
            "{name}: FnPtr::new(metaloadfn(\"{symbol}\", &[{fallbacks}])),",
            name = cmd.proto.ident,
            symbol = generators::gen_symbol_name(registry.api, &cmd.proto.ident),
            fallbacks = match registry.aliases.get(&cmd.proto.ident) {
                Some(fbs) => fbs
                    .iter()
                    .map(|name| format!("\"{}\"", generators::gen_symbol_name(registry.api, &name)))
                    .collect::<Vec<_>>()
                    .join(", "),
                None => format!(""),
            },
        ))
    }

    writeln!(dest, "}},")?;

    try!(writeln!(dest, "_priv: ()"));

    try!(writeln!(
        dest,
        "}}
        }}"
    ));

    for cmd in &registry.cmds {
        try!(writeln!(dest,
            "#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn {name}(&self, {params}) -> {return_suffix} {{ \
                __gl_imports::mem::transmute::<_, extern \"system\" fn({typed_params}) -> {return_suffix}>\
                    (self.ptrs.{name}.f)({idents}) \
            }}",
            name = cmd.proto.ident,
            params = gen_parameters(cmd, &registry, true, true).join(", "),
            typed_params = gen_parameters(cmd, &registry, false, true).join(", "),
            return_suffix = cmd.proto.ty,
            idents = gen_parameters(cmd, &registry, true, false).join(", "),
        ))
    }

    writeln!(
        dest,
        "}}

        unsafe impl __gl_imports::Send for {api} {{}}",
        api = generators::gen_struct_name(registry.api)
    )
}

fn write_enum_groups<W>(registry: &Registry, dest: &mut W) -> io::Result<()>
    where W: io::Write
{
    writeln!(dest, "macro_rules! impl_enum_traits {{
        ($Name:ident) => {{

        }}
    }}")?;
    writeln!(dest, "")?;

    writeln!(dest, "macro_rules! impl_enum_bitmask_traits {{
        ($Name:ident) => {{
            
        }}
    }}")?;
    writeln!(dest, "")?;

    let mut enums = ::std::collections::HashSet::new();

    for en in registry.enums.iter() {
        enums.insert(en.ident.as_str());
    }

    writeln!(dest, "pub mod enums {{")?;

    // NOTE: unreachable_patterns is allowed due to enum aliases
    writeln!(dest, "#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case, dead_code, unreachable_patterns)]")?;

    writeln!(dest, "")?;
    writeln!(dest, "use super::types;")?;
    writeln!(dest, "")?;

    for (_, group) in registry.groups.iter() {

        let enum_type = if group.ident == "Boolean" {
            "types::GLboolean"
        } else {
            "types::GLenum"
        };

        writeln!(dest, "#[repr(transparent)]")?;
        writeln!(dest, "#[derive(Copy, Clone, PartialEq, Eq, Hash)]")?;
        writeln!(dest, "pub struct {}(pub {});", group.ident, enum_type)?;
        writeln!(dest, "")?;
        writeln!(dest, "impl {} {{", group.ident)?;

        let mut group_enums = ::std::collections::HashSet::new();

        for enum_name in group.enums.iter() {
            let unique = group_enums.insert(enum_name.as_str());
            if unique && enums.contains(enum_name.as_str()) {
                writeln!(dest, "    pub const {enum_name}: {group_name} = {group_name}(super::{enum_name});", 
                    group_name = group.ident, enum_name = enum_name)?;
            }
        }
        
        if let Some("bitmask") = group.enums_type.as_ref().map(|t| t.as_str()) {
            writeln!(dest, "    pub const Empty: {group_name} = {group_name}(0);", 
                group_name = group.ident)?;
        }

        writeln!(dest, "}}")?;
        writeln!(dest, "")?;

        writeln!(dest, "impl ::std::fmt::Debug for {} {{", group.ident)?;
        writeln!(dest, "    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {{")?;
        writeln!(dest, "        match *self {{")?;
        for enum_name in group_enums.iter() {
            if enums.contains(enum_name) {
                writeln!(dest, "            {group_name}::{enum_name} => write!(fmt, \"{group_name}({enum_name})\"),", 
                    group_name = group.ident, enum_name = enum_name)?;
            }
        }
        writeln!(dest, "            _ => write!(fmt, \"{group_name}({{}})\", self.0),", group_name = group.ident)?;
        writeln!(dest, "        }}")?;
        writeln!(dest, "    }}")?;
        writeln!(dest, "}}")?;
        writeln!(dest, "")?;

        writeln!(dest, "impl_enum_traits!({});", group.ident)?;
        writeln!(dest, "")?;

        if let Some("bitmask") = group.enums_type.as_ref().map(|t| t.as_str()) {
            writeln!(dest, "impl_enum_bitmask_traits!({});", group.ident)?;
            writeln!(dest, "")?;
        }
    }
    
    writeln!(dest, "}}")?;

    Ok(())
}

/// Generates the list of Rust `Arg`s that a `Cmd` requires.
pub fn gen_parameters(cmd: &Cmd, registry: &Registry, with_idents: bool, with_types: bool) -> Vec<String> {
    cmd.params
        .iter()
        .map(|binding| {
            let ty = binding.group
                .as_ref()
                .and_then(|group| registry.groups.get(group).map(|group| format!("enums::{}", group.ident)))
                .unwrap_or(binding.ty.to_string());

            // returning
            if with_idents && with_types {
                format!("{}: {}", binding.ident, ty)
            } else if with_types {
                format!("{}", ty)
            } else if with_idents {
                format!("{}", binding.ident)
            } else {
                panic!()
            }
        })
        .collect()
}