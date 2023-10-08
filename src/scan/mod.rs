use std::thread; 
use crate::SharedRunState;
use std::sync::mpsc::{Receiver, Sender};

use self::{
    cookie::CookieHasher,
    logger::Logger,
    packet_receiver::PacketReceiver,
    packet_sender::PacketSender,
    network_data::InterfaceData, output::{OutputHandle, Output},
};

pub mod cookie;
pub mod lcg;
pub mod logger;
pub mod packet_receiver;
pub mod packet_sender;
pub mod output;
pub mod network_data;

pub struct Scanner {
    packet_sender: PacketSender,
    packet_receiver: PacketReceiver,
    packet_receiver_control_tx: ControlTx, 
    logger: Logger,
    logger_control_tx: ControlTx,
    output: Box<dyn Output + Send>,
    output_control_tx: ControlTx
}

pub enum ThreadControlMessage {
    Die, 
}

pub type ControlTx = Sender<ThreadControlMessage>; 
pub type ControlRx = Receiver<ThreadControlMessage>;

impl Scanner {
    pub(crate) fn new(port: u16, interface_data: InterfaceData, run_state: SharedRunState, output: Box<dyn Output + Send>, output_handle: OutputHandle) -> Self {
        let cookie_hasher = CookieHasher::new();

        let (logger, stats, logger_control_tx) = Logger::new(run_state.clone());

        let packet_sender = PacketSender::new(
            cookie_hasher.clone(),
            port,
            interface_data.clone(),
            stats.clone(),
            run_state.clone(),
        );

        let (packet_receiver, packet_receiver_control_tx) = PacketReceiver::new(
            cookie_hasher.clone(),
            port,
            interface_data.clone(),
            stats.clone(),
            run_state,
            output_handle.out_tx, 
        );


        let scanner = Scanner {
            packet_sender,
            packet_receiver_control_tx, 
            packet_receiver,
            logger,
            logger_control_tx,
            output,
            output_control_tx: output_handle.control_tx
        };
        
        scanner
    }
    pub(crate) fn scan(mut self) {
        let output_handle = thread::spawn(move || self.output.output()); 
        let receiver_handle = thread::spawn(move || self.packet_receiver.receive());
        let sender_handle = thread::spawn(move || self.packet_sender.send());
        let logger_handle = thread::spawn(move || self.logger.log());

        sender_handle.join().unwrap();
        self.packet_receiver_control_tx.send(ThreadControlMessage::Die).unwrap();
        receiver_handle.join().unwrap();
        self.logger_control_tx.send(ThreadControlMessage::Die).unwrap(); 
        logger_handle.join().unwrap();
        self.output_control_tx.send(ThreadControlMessage::Die).unwrap(); 
        output_handle.join().unwrap();
    }
}
