use std::path::PathBuf;

use crate::function::FunctionSpec;
use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Manually specify the path to the config.
    #[arg(short)]
    from_config: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {
    /// The port the daemon should bind to on your computer.
    pub port: u16,
    /// Default state the daemon should start with.
    pub initial: Vec<f32>,
    /// Who you want to send your output to whenever it's updated. Include both ip an port please.
    pub downstream: Vec<String>,
    /// What kind of function should the daemon run? It needs to know how it should load it after all.
    /// You can make dynamic libraries and specify the symbols, or run some arbitrary executable and
    /// define how it accepts data.
    pub function_spec: FunctionSpec,
}

pub fn get_config() -> Config {
    let args = Args::parse();

    let config_path = if let Some(path) = args.from_config {
        PathBuf::from(path)
    } else {
        let mut home_dir = dirs::home_dir().unwrap();
        #[cfg(target_os = "windows")]
        {
            home_dir.push("AppData");
            home_dir.push("Roaming");
        }
        #[cfg(not(target_os = "windows"))]
        home_dir.push(".config");

        home_dir.push("daimon");
        home_dir.push("config");
        home_dir.set_extension("toml");
        home_dir
    };

    let config = std::fs::read_to_string(config_path).unwrap();
    ron::from_str(&config).unwrap()
}
