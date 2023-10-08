#![feature(core_intrinsics)]
#![feature(ip_bits)]
#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(allocator_api)]

use std::io::stdin;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use clap::Parser;
use scan::output::FileOut;

#[derive(Parser, Debug)]
#[command(name = "Sint")]
#[command(version = "1.0")]
#[command(about = "Scanner that scans !")]
struct Args {
    /// Output file
    #[arg(short = 'o', long = "output")]
    output: String,
    #[arg(short = 'p', long = "port")]
    /// Port to be scanned
    port: u16,
    #[arg(short = 'i', long = "interface")]
    /// Interface to scan on
    interface: Option<String>,
}

mod scan;

pub struct RunState {
    pub paused: Mutex<bool>,
    pub cond: Condvar,
}

impl RunState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            paused: Mutex::new(false),
            cond: Condvar::new(),
        })
    }

    pub fn act_state(&self) {
        if *self.paused.lock().unwrap() {
            self.cond
                .wait_while(self.paused.lock().unwrap(), |paused| *paused)
                .unwrap();
        }
    }

    fn toogle(&self) {
        let mut paused = self.paused.lock().unwrap();
        self.cond.notify_all();
        *paused = !*paused;
    }
}

type SharedRunState = Arc<RunState>;

fn main() {
    let args = Args::parse();

    let interface_data;
    match args.interface {
        Some(interface) => {
            interface_data = scan::network_data::InterfaceData::fetch_from_interface(&interface)
        }
        None => interface_data = scan::network_data::InterfaceData::fetch_default(),
    }

    let run_state = RunState::new();

    let (output, output_handle) = FileOut::new(args.output);

    let scanner = scan::Scanner::new(
        args.port,
        interface_data,
        run_state.clone(),
        Box::new(output),
        output_handle,
    );
    let scan = thread::spawn(|| scanner.scan());

    let stdin = stdin();

    let mut buf = String::new();
    loop {
        stdin.read_line(&mut buf).unwrap();
        run_state.toogle();
        if scan.is_finished() {
            break;
        }
    }
}
