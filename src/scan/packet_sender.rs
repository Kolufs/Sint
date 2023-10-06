use std::{net::Ipv4Addr, thread};

use super::{cookie::CookieHasher, lcg, logger::LoggerStats, utils::InterfaceData};
use flume::{bounded, Receiver, Sender};
use lcg::IPv4Iterator;

use pnet_packet::{ethernet::EtherTypes, ip::IpNextHeaderProtocols, tcp::TcpFlags};

use pnet::datalink;
use pnet::packet;

extern crate pretty_env_logger;
pub(crate) struct PacketSender {
    ipv4_iterator: IPv4Iterator,
    channel: Box<dyn datalink::DataLinkSender>,
    cookie_hasher: CookieHasher,
    interface_data: super::utils::InterfaceData,
    rendezvous_control_rx: Receiver<PacketSenderControlMessage>,
    stats: LoggerStats,
}

pub(crate) enum PacketSenderControlMessage {
    Die,
    Pause,
}

pub(crate) struct PacketSenderHandler {
    pub(crate) rendezvous_control_tx: Sender<PacketSenderControlMessage>,
}

impl PacketSender {
    pub fn new(
        cookie_hasher: CookieHasher,
        port: u16,
        interface_data: InterfaceData,
        stats: LoggerStats,
    ) -> (PacketSender, PacketSenderHandler) {
        let test = pnet::datalink::Config {
            write_buffer_size: 4096,
            read_buffer_size: 4096,
            read_timeout: None,
            write_timeout: None,
            channel_type: pnet_datalink::ChannelType::Layer2,
            bpf_fd_attempts: 1000,
            linux_fanout: None,
            promiscuous: false,
        };

        let pnet_channel = datalink::channel(&interface_data.iface, test).unwrap();

        let ipv4_iterator: IPv4Iterator = IPv4Iterator::new(port);

        let (tx, _) = match pnet_channel {
            pnet_datalink::Channel::Ethernet(sender, receiver) => (sender, receiver),
            _ => panic!(),
        };

        let (rendezvous_control_tx, rendezvous_control_rx) = bounded(0);

        (
            PacketSender {
                ipv4_iterator,
                channel: tx,
                cookie_hasher,
                interface_data,
                rendezvous_control_rx,
                stats,
            },
            PacketSenderHandler {
                rendezvous_control_tx,
            },
        )
    }

    fn make_packet(&self, dst_ip: Ipv4Addr, src_port: u16, dst_port: u16, buffer: &mut [u8]) {
        {
            let mut eth_header =
                packet::ethernet::MutableEthernetPacket::new(&mut buffer[0..14]).unwrap();
            eth_header.set_destination(self.interface_data.gateway_mac);
            eth_header.set_source(self.interface_data.iface.mac.unwrap());
            eth_header.set_ethertype(EtherTypes::Ipv4);
        }
        {
            let mut ip_header = packet::ipv4::MutableIpv4Packet::new(&mut buffer[14..34]).unwrap();
            ip_header.set_source(self.interface_data.device_ip.clone());
            ip_header.set_destination(dst_ip.clone());
            ip_header.set_header_length(5);
            ip_header.set_total_length(40);
            ip_header.set_next_level_protocol(IpNextHeaderProtocols::Tcp);
            ip_header.set_identification(5);
            ip_header.set_ttl(64);
            ip_header.set_version(4);

            let checksum = pnet_packet::ipv4::checksum(&ip_header.to_immutable());
            ip_header.set_checksum(checksum);
        }
        {
            let mut tcp_header = packet::tcp::MutableTcpPacket::new(&mut buffer[34..54]).unwrap();
            tcp_header.set_source(src_port);
            tcp_header.set_destination(dst_port);
            tcp_header.set_flags(TcpFlags::SYN);
            tcp_header.set_window(64240);
            tcp_header.set_data_offset(5);

            tcp_header.set_checksum(0);
            let checksum = pnet_packet::tcp::ipv4_checksum(
                &tcp_header.to_immutable(),
                &self.interface_data.device_ip,
                &dst_ip,
            );
            tcp_header.set_checksum(checksum);
        }
    }

    pub fn send(&mut self) {
        let mut packet_data: [u8; 54] = [0u8; 54];
        while let Some(curr_addr) = self.ipv4_iterator.next() {
            if let Ok(message) = self.rendezvous_control_rx.try_recv() {
                match message {
                    PacketSenderControlMessage::Die => {
                        return;
                    }
                    PacketSenderControlMessage::Pause => {
                        thread::park();
                    }
                }
            }
            let hash = self.cookie_hasher.get_port_cookie(
                self.interface_data.device_ip.clone(),
                curr_addr.ip().clone(),
            );
            self.make_packet(
                curr_addr.ip().clone(),
                hash,
                curr_addr.port(),
                &mut packet_data,
            );
            self.channel.send_to(&packet_data, None);

            self.stats.lock().unwrap().sent += 1;
        }
    }
}
