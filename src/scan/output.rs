use std::io::Write;
use std::net::Ipv4Addr;
use std::sync::mpsc::{channel, Receiver, Sender};

use std::fs;
use std::time::Duration;

use super::{ControlRx, ControlTx};

pub type OutTx = Sender<Ipv4Addr>;
pub type OutRx = Receiver<Ipv4Addr>;

pub trait Output {
    fn output(&mut self);
}

pub struct OutputHandle {
    pub out_tx: OutTx,
    pub control_tx: ControlTx,
}

pub struct FileOut {
    file_handle: fs::File,
    out_rx: OutRx,
    control_rx: ControlRx,
}

impl FileOut {
    pub fn new(path: String) -> (Self, OutputHandle) {
        let file_handle = match fs::OpenOptions::new().create(true).write(true).open(path) {
            Ok(file) => file,
            Err(err) => panic!("Failed opening file: {}", err),
        };

        let (out_tx, out_rx) = channel();
        let (control_tx, control_rx) = channel();

        let output_handle = OutputHandle { out_tx, control_tx };

        (
            FileOut {
                file_handle,
                out_rx,
                control_rx,
            },
            output_handle,
        )
    }
}

impl Output for FileOut {
    fn output(&mut self) {
        loop {
            if let Ok(_) = self.control_rx.try_recv() {
                return;
            }
            if let Ok(socket) = self.out_rx.recv_timeout(Duration::new(1, 0)) {
                self.file_handle
                    .write_all(format!("{}\n", socket.to_string()).as_bytes())
                    .unwrap();
            }
        }
    }
}
