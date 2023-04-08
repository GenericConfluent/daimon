use clap::ValueEnum;
use core::ffi::{c_float, c_ulonglong};
use serde::Deserialize;

/// Describes how the function can be dealt with.
#[derive(Clone, Deserialize, ValueEnum)]
pub enum FunctionSpec {
    Library,
    Exec,
}

/// # Passing data to your handler
/// The `Binary` variant will pass a blob of binary data encoded with the rust `bincode` crate. The
/// representation is compact and quick, but may require significant effort to parse depending on
/// which language you are using. Which is why the default format(`Arg`) exists. It simply passes each
/// float in the input vector to the function.
#[derive(Default, Clone, Deserialize, ValueEnum)]
pub enum Format {
    Binary,
    #[default]
    Arg,
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
}

impl From<crate::Config> for Function {
    fn from(config: crate::Config) -> Function {
        config
            .spec
            .and_then(|function_spec| {
                Some(match function_spec {
                    FunctionSpec::Library => unsafe {
                        let lib = libloading::Library::new(config.path.unwrap())
                            .expect("Invalid library path");
                        Function::Library {
                            lib,
                            symbol: config.symbol.unwrap(),
                        }
                    },
                    FunctionSpec::Exec => {
                        todo!()
                    }
                })
            })
            .unwrap_or_default()
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
        }
    }
}
