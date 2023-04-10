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
#![feature(iter_intersperse)]
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    net::{TcpListener, TcpStream},
};

mod function;
use function::Function;

/// Utility module for command line arg parsing using clap (suprisingly).
mod config;

#[derive(Serialize, Deserialize)]
enum Event {
    Output(Vec<f32>),
    RequestOutput,
}

fn send_recipients(recipients: &Vec<String>, outputs: &Vec<f32>) {
    let message = Event::Output(outputs.clone());
    let serialized = bincode::serialize(&message).expect("Could not serialize response");
    for address in recipients {
        let mut stream = TcpStream::connect(address).expect("Recipient address wrongly formatted");
        stream
            .write_all(&serialized)
            .expect("Could not write data to stream");
    }
}

fn main() {
    let config::Config {
        port,
        initial: mut outputs,
        downstream,
        function_spec,
    } = config::get_config();

    // let mut outputs = config.initial;
    let function = Function::from(function_spec);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).expect("Could not bind socket");
    if !outputs.is_empty() {
        send_recipients(&downstream, &outputs);
    }

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let event: Event = bincode::deserialize_from(&stream).unwrap();
        match event {
            Event::Output(inputs) => {
                outputs = function(inputs);
                send_recipients(&downstream, &outputs);
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
