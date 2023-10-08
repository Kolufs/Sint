use std::net::IpAddr;
use std::sync::mpsc::channel;

use crate::SharedRunState;

use super::output::OutTx;
use super::{cookie::CookieHasher, logger::LoggerStats, network_data::InterfaceData};
use super::{ControlRx, ControlTx};

use pnet_packet::{ip::IpNextHeaderProtocols, tcp::TcpFlags};

use std::time::Duration;

use pnet::transport;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::Ipv4;

pub(crate) struct PacketReceiver {
    layer_receiver: transport::TransportReceiver,
    cookie_hasher: CookieHasher,
    interface_data: InterfaceData,
    port: u16,
    stats: LoggerStats,
    run_state: SharedRunState,
    control_rx: ControlRx,
    out_tx: OutTx,
}

impl PacketReceiver {
    pub fn new(
        cookie_hasher: CookieHasher,
        port: u16,
        interface_data: InterfaceData,
        stats: LoggerStats,
        run_state: SharedRunState,
        out_tx: OutTx,
    ) -> (PacketReceiver, ControlTx) {
        let (_tx, layer_receiver) =
            transport::transport_channel(64553, Layer4(Ipv4(IpNextHeaderProtocols::Tcp))).unwrap();

        let (control_tx, control_rx) = channel();

        let packet_receiver = PacketReceiver {
            layer_receiver,
            cookie_hasher,
            interface_data,
            port,
            stats,
            run_state,
            control_rx,
            out_tx,
        };

        (packet_receiver, control_tx)
    }

    pub fn receive(&mut self) {
        let mut packet_rcv: pnet_transport::TcpTransportChannelIterator<'_> =
            pnet::transport::tcp_packet_iter(&mut self.layer_receiver);

        loop {
            self.run_state.act_state();
            if let Ok(_) = self.control_rx.try_recv() {
                return;
            }
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
                                self.stats.lock().unwrap().received += 1;
                                self.out_tx.send(src_ip).unwrap();
                            } else {
                                continue;
                            }
                        }
                    }
                }
                Err(err) => println!("{}", err),
            }
        }
    }
}
