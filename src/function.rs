use core::ffi::{c_float, c_ulonglong};
use serde::Deserialize;
use std::{io::Write, path::PathBuf, process::Stdio};

/// Describes how the function can be dealt with.
#[derive(Deserialize)]
pub enum FunctionSpec {
    Echo,
    Lib {
        lib_path: String,
        symbol: String,
    },
    Exec {
        exec_path: String,
        input: exec::Interface,
        output: exec::Interface,
    },
}

mod exec {
    use super::*;
    /// # Passing data to your handler
    /// The `Binary` variant will pass a blob of binary data encoded with the rust `bincode` crate. The
    /// representation is compact and quick, but may require significant effort to parse depending on
    /// which language you are using. Which is why the default format(`Arg`) exists. It simply passes each
    /// float in the input vector to the function.
    #[derive(Default, Clone, Deserialize)]
    pub enum Format {
        Binary,
        #[default]
        Plain,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub enum LocationDescriptor {
        CmdArgs,
        File(std::path::PathBuf),
        Stdin,
        Stdout,
    }

    #[derive(Deserialize)]
    pub struct Interface {
        pub location: LocationDescriptor,
        /// How should the input vector be passed to the function. See [`Format`] for more details.
        pub format: Format,
    }
}

type LibraryFunction<'lib> = libloading::Symbol<
    'lib,
    unsafe extern "C" fn(*const c_float, c_ulonglong, *mut c_float, *mut c_ulonglong),
>;

#[derive(Default)]
pub enum Function {
    /// Default behaviour for the daemon. Take recieve state from inputs and propagate to ouputs.
    #[default]
    Echo,
    /// Function was loaded from some compiled system library.
    Library {
        lib: libloading::Library,
        symbol: String,
    },
    Exec {
        input: exec::Interface,
        output: exec::Interface,
        exec_path: std::path::PathBuf,
    },
}

impl From<FunctionSpec> for Function {
    fn from(function_spec: FunctionSpec) -> Function {
        match function_spec {
            FunctionSpec::Echo => Function::Echo,
            FunctionSpec::Lib { lib_path, symbol } => unsafe {
                let lib = libloading::Library::new(lib_path).expect("Invalid library path");
                Function::Library { lib, symbol }
            },
            FunctionSpec::Exec {
                exec_path,
                input,
                output,
            } => Function::Exec {
                input,
                output,
                exec_path: PathBuf::from(exec_path),
            },
        }
    }
}

type Args = (Vec<f32>,);

impl FnOnce<Args> for Function {
    type Output = Vec<f32>;
    extern "rust-call" fn call_once(self, args: Args) -> Self::Output {
        self.call(args)
    }
}

impl FnMut<Args> for Function {
    extern "rust-call" fn call_mut(&mut self, args: Args) -> Self::Output {
        self.call(args)
    }
}

use exec::Format;

impl Fn<Args> for Function {
    extern "rust-call" fn call(&self, args: Args) -> Self::Output {
        let mut input = args.0;
        match self {
            Function::Echo => input,
            Function::Library { lib, symbol } => unsafe {
                let inner: LibraryFunction = lib.get(symbol.as_bytes()).unwrap();
                let result_ptr: *mut c_float = std::ptr::null_mut();
                let result_len: *mut c_ulonglong = std::ptr::null_mut();
                inner(
                    input.as_mut_ptr() as _,
                    input.len() as c_ulonglong,
                    result_ptr,
                    result_len,
                );
                let result_len = result_len as usize;
                Vec::from_raw_parts(result_ptr, result_len, result_len)
            },
            Function::Exec {
                input: exec_input,
                output: _exec_output,
                exec_path,
            } => {
                let (exec_args, stdin) = match exec_input.location {
                    exec::LocationDescriptor::CmdArgs => match exec_input.format {
                        Format::Binary => {
                            panic!("Exec cannot take an arg with binary encoding. Consider using Format::Plain instead.");
                        }
                        Format::Plain => {
                            let args = input.iter().map(f32::to_string).collect::<Vec<String>>();
                            (args, vec![])
                        }
                    },
                    exec::LocationDescriptor::Stdin => match exec_input.format {
                        Format::Binary => {
                            let stdin = bincode::serialize(&input).unwrap();
                            (vec![], stdin)
                        }
                        Format::Plain => {
                            let stdin = input.iter().map(f32::to_string).collect::<Vec<String>>();

                            let stdin = stdin
                                .iter()
                                .flat_map(|element| element.bytes())
                                .intersperse(b'\n')
                                .collect::<Vec<u8>>();
                            (vec![], stdin)
                        }
                    },
                    ref unsuported => {
                        panic!("Sorry. You cannot take exec input with {:?}", unsuported)
                    }
                };

                let mut process = std::process::Command::new(exec_path)
                    .args(exec_args)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .spawn()
                    .expect("Failed to execute process.");

                if !stdin.is_empty() {
                    process
                        .stdin
                        .as_mut()
                        .unwrap()
                        .write_all(&stdin)
                        .expect("Could not write data to stdin");
                }

                let stdout = process
                    .wait_with_output()
                    .expect("Could not get process stdout")
                    .stdout;

                String::from_utf8_lossy(&stdout)
                    .split_whitespace()
                    .map(|element| str::parse::<f32>(element).unwrap())
                    .collect()
            }
        }
    }
}
