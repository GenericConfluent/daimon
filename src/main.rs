//! # 大門
//! This is a simple daemon that makes it easy and convenient to thread together a series of
//! computations sequentially across a number of computers. There are basically two ways to use
//! this:
//! 1. Get yourself a couple computers and configure each with the necessary settings and functions
//!    to apply some transformation that is valuable to you.
//! 2. Make this basically a webserver and put out your port and ip online.
//! I would highly not recommend option #2 because it's appears inherently dangerous in more ways
//! than one.
//!
//! # Setup
//! There are two ways you can go about running this thing. The first is to create a config file
//! located in the present working directory and the second is to call the daemon with arguments.
//! See [`Config`] or run `daimon -h` to read more about configuration.
#![feature(iter_collect_into)]
#![feature(unboxed_closures)]
#![feature(fn_traits)]
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

mod function;
use function::{Format, Function, FunctionSpec};

#[derive(Serialize, Deserialize)]
enum Event {
    Output(Vec<f32>),
    RequestOutput,
}

#[derive(Deserialize, Parser)]
#[command(author, version, about)]
struct Config {
    /// The port the daemon should bind to on your computer.
    #[arg(short, long, default_value_t = 7135)]
    port: u16,
    /// Default state the daemon should start with.
    #[arg(long)]
    initial: Vec<f32>,
    /// Setup program from config file.
    #[arg(long, exclusive = true)]
    from_config: Option<String>,
    /// What kind of function should the daemon run? It needs to know how it should load it after all.
    /// You can make dynamic libraries and specify the symbols, or run some arbitrary executable and
    /// define how it accepts data.
    #[arg(long)]
    spec: Option<FunctionSpec>,
    /// Where can the file object that holds the function be found? Should be a library or executable.
    #[arg(long)]
    path: Option<String>,
    #[arg(long)]
    symbol: Option<String>,
    /// How should the input vector be passed to the function. See [`Format`] for more details.
    #[arg(long)]
    format: Option<Format>,
    /// Who you want to send your output to whenever it's updated. Include both ip an port please.
    #[arg(short, long, required = true)]
    recipients: Vec<String>,
}

fn send_recipients(recipients: &Vec<String>, outputs: &Vec<f32>) {
    let message = Event::Output(outputs.clone());
    let serialized = bincode::serialize(&message).expect("Could not serialize response");
    for address in recipients {
        let mut stream = TcpStream::connect(address).expect("Recipient address wrongly formatted");
        stream.write(&serialized).expect("Could not write data to stream");
    }
}

fn main() {
    let args = Config::parse();
    let config: Config = match args.from_config.as_ref().and_then(|config_path| File::open(config_path).ok()) {
        Some(mut config_file) => {
            let mut buffer = String::new();
            config_file.read_to_string(&mut buffer).unwrap();
            ron::from_str(&buffer).unwrap()
        }
        None => args
    };

    let mut outputs = config.initial.clone();
    let recipients = config.recipients.clone();
    let port = config.port;
    let function = Function::from(config);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).expect("Could not bind socket");
    if outputs.len() > 0 {
        send_recipients(&recipients, &outputs);
    }

    
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let event: Event = bincode::deserialize_from(&stream).unwrap();
        match event {
            Event::Output(inputs) => {
                outputs = function(inputs);
                send_recipients(&recipients, &outputs);
            }
            Event::RequestOutput => {
                let output = Event::Output(outputs.clone());
                let output = bincode::serialize(&output).expect("Could not serialize response");
                stream
                    .write_all(&output)
                    .expect("Failed to write output to TcpStream");
                stream.flush().expect("Error sending outputs");
            }
        }
    }
}
