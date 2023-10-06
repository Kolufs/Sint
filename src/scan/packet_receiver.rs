use std::{
    net::{IpAddr, SocketAddr},
    thread,
};

use super::{cookie::CookieHasher, logger::LoggerStats, utils::InterfaceData};

use pnet_packet::{ip::IpNextHeaderProtocols, tcp::TcpFlags};

use std::time::Duration;

use pnet::transport;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::Ipv4;

use flume::{bounded, unbounded, Receiver, Sender};

extern crate pretty_env_logger;

pub(crate) enum PacketReceiverControlMessage {
    Die,
    Pause,
}

pub(crate) struct PacketReceiver {
    layer_receiver: transport::TransportReceiver,
    cookie_hasher: CookieHasher,
    interface_data: InterfaceData,
    rendezvous_control_rx: Receiver<PacketReceiverControlMessage>,
    product_tx: Sender<SocketAddr>,
    port: u16,
    stats: LoggerStats,
}

pub(crate) struct PacketReceiverHandler {
    pub(crate) rendezvous_control_tx: Sender<PacketReceiverControlMessage>,
    pub(crate) product_rx: Receiver<SocketAddr>,
}

impl PacketReceiver {
    pub fn new(
        cookie_hasher: CookieHasher,
        port: u16,
        interface_data: InterfaceData,
        stats: LoggerStats,
    ) -> (PacketReceiver, PacketReceiverHandler) {
        let (_tx, layer_receiver) =
            transport::transport_channel(64553, Layer4(Ipv4(IpNextHeaderProtocols::Tcp))).unwrap();

        let (rendezvous_control_tx, rendezvous_control_rx) = bounded(0);
        let (product_tx, packet_receiver_product_rx) = unbounded();

        let packet_receiver_handler = PacketReceiverHandler {
            rendezvous_control_tx,
            product_rx: packet_receiver_product_rx,
        };

        let packet_receiver = PacketReceiver {
            layer_receiver,
            cookie_hasher,
            rendezvous_control_rx,
            product_tx,
            interface_data,
            port,
            stats,
        };

        (packet_receiver, packet_receiver_handler)
    }

    pub fn receive(&mut self) {
        let mut packet_rcv: pnet_transport::TcpTransportChannelIterator<'_> =
            pnet::transport::tcp_packet_iter(&mut self.layer_receiver);

        loop {
            match packet_rcv.next_with_timeout(Duration::new(5, 0)) {
                Ok(res) => {
                    if let Some((packet, addr)) = res {
                        let src_port = packet.get_source();
                        if !self.port == src_port {
                            continue;
                        }

                        let src_ip = match addr {
                            IpAddr::V4(ip) => ip,
                            IpAddr::V6(_) => continue,
                        };

                        let dst_ip = self.interface_data.device_ip;
                        let dst_port = packet.get_destination();

                        let flags = packet.get_flags();

                        if flags & TcpFlags::SYN == TcpFlags::SYN
                            && flags & TcpFlags::ACK == TcpFlags::ACK
                        {
                            if self
                                .cookie_hasher
                                .check_port_cookie(dst_ip, src_ip, dst_port)
                            {
                                let addr = SocketAddr::new(std::net::IpAddr::V4(src_ip), src_port);
                                self.stats.lock().unwrap().received += 1;
                                self.product_tx.send(addr).unwrap();
                            } else {
                                continue;
                            }
                        }
                    }
                }
                Err(err) => println!("{}", err),
            }
            if let Ok(message) = self.rendezvous_control_rx.try_recv() {
                match message {
                    PacketReceiverControlMessage::Die => {
                        debug!("Died");
                        return;
                    }
                    PacketReceiverControlMessage::Pause => {
                        debug!("Paused");
                        thread::park();
                        debug!("Unpaused");
                    }
                }
            }
        }
    }
}
