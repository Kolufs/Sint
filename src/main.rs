#![feature(core_intrinsics)]
#![feature(ip_bits)]

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use std::fs;
use std::io::Write;
use std::thread;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(term_width = 0)]
struct Args {
    max_kbps: Option<u32>,
    output: String,
    port: u16,
    interface: Option<String>,
}

mod scan;

fn main() {
    pretty_env_logger::init();

    let args = Args::parse();

    let interface_data;
    match args.interface {
        Some(interface) => {
            interface_data = scan::utils::InterfaceData::fetch_from_interface(&interface)
        }
        None => interface_data = scan::utils::InterfaceData::fetch_default(),
    }

    let (scanner, scanner_handler) = scan::Scanner::new(args.port, interface_data);

    let scan = thread::spawn(|| scanner.scan());

    let mut output_handle = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(args.output)
        .unwrap();

    while let Ok(addr) = scanner_handler.product_rx.recv() {
        output_handle
            .write(format!("{}\n", addr.to_string()).as_bytes())
            .unwrap();
    }

    scan.join().unwrap();
}
