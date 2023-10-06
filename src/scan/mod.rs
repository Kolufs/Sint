use flume::{bounded, Receiver, Sender};
use std::{net::SocketAddr, thread, time::Duration};

use self::{
    cookie::CookieHasher,
    logger::{Logger, LoggerControlMessage, LoggerHandler},
    packet_receiver::{PacketReceiver, PacketReceiverHandler},
    packet_sender::{PacketSender, PacketSenderHandler},
    utils::InterfaceData,
};

extern crate pretty_env_logger;

pub mod cookie;
pub mod lcg;
pub mod logger;
pub mod packet_receiver;
pub mod packet_sender;
pub mod utils;

#[derive(Debug)]
pub enum ScannerControlMessage {
    Pause,
    Die,
}

pub struct Scanner {
    packet_sender: PacketSender,
    packet_sender_handler: PacketSenderHandler,
    packet_receiver: PacketReceiver,
    packet_receiver_handler: PacketReceiverHandler,
    logger: Logger,
    logger_handler: LoggerHandler,
    rendezvous_control_rx: Receiver<ScannerControlMessage>,
    product_tx: Sender<SocketAddr>,
}

#[derive(Debug)]
pub(crate) struct ScannerHandler {
    pub(crate) rendezvous_control_tx: Sender<ScannerControlMessage>,
    pub(crate) product_rx: Receiver<SocketAddr>,
}

impl Scanner {
    pub(crate) fn new(port: u16, interface_data: InterfaceData) -> (Self, ScannerHandler) {
        let cookie_hasher = CookieHasher::new();

        let (logger, stats, logger_handler) = Logger::new();

        let (packet_sender, packet_sender_handler) = PacketSender::new(
            cookie_hasher.clone(),
            port,
            interface_data.clone(),
            stats.clone(),
        );
        let (packet_receiver, packet_receiver_handler) = PacketReceiver::new(
            cookie_hasher.clone(),
            port,
            interface_data.clone(),
            stats.clone(),
        );

        let (rendezvous_control_tx, rendezvous_control_rx) = bounded(0);

        let (product_tx, product_rx) = bounded(5);

        let scanner_handler = ScannerHandler {
            rendezvous_control_tx,
            product_rx,
        };

        let scanner = Scanner {
            packet_sender,
            packet_sender_handler,
            packet_receiver,
            packet_receiver_handler,
            logger,
            logger_handler,
            rendezvous_control_rx,
            product_tx,
        };

        (scanner, scanner_handler)
    }

    pub(crate) fn scan(mut self) {
        let receiver_handle = thread::spawn(move || self.packet_receiver.receive());
        let sender_handle = thread::spawn(move || self.packet_sender.send());
        let logger_handle = thread::spawn(move || self.logger.log());

        while !sender_handle.is_finished() {
            if let Ok(socket) = self.packet_receiver_handler.product_rx.try_recv() {
                self.product_tx.send(socket).unwrap();
            }

            if let Ok(message) = self.rendezvous_control_rx.try_recv() {
                match message {
                    ScannerControlMessage::Pause => {
                        self.packet_sender_handler
                            .rendezvous_control_tx
                            .send(packet_sender::PacketSenderControlMessage::Pause)
                            .unwrap();

                        std::thread::sleep(Duration::new(5, 0));

                        self.packet_receiver_handler
                            .rendezvous_control_tx
                            .send(packet_receiver::PacketReceiverControlMessage::Pause)
                            .unwrap();

                        self.logger_handler
                            .send(LoggerControlMessage::Pause)
                            .unwrap();

                        thread::park();
                        receiver_handle.thread().unpark();
                        sender_handle.thread().unpark();
                        logger_handle.thread().unpark();
                    }
                    ScannerControlMessage::Die => {
                        self.packet_sender_handler
                            .rendezvous_control_tx
                            .send(packet_sender::PacketSenderControlMessage::Die)
                            .unwrap();

                        sender_handle.join().unwrap();
                        break;
                    }
                }
            }
        }

        self.packet_receiver_handler
            .rendezvous_control_tx
            .send(packet_receiver::PacketReceiverControlMessage::Die)
            .unwrap();

        self.logger_handler.send(LoggerControlMessage::Die).unwrap();

        receiver_handle.join().unwrap();
        logger_handle.join().unwrap();
    }
}
