use pnet::ipnetwork::IpNetwork;
use pnet_datalink::MacAddr;
use std::{fs, net::Ipv4Addr, str::FromStr};

#[derive(Clone)]
pub struct InterfaceData {
    pub iface: pnet_datalink::NetworkInterface,
    pub gateway_mac: MacAddr,
    pub gateway_ip: Ipv4Addr,
    pub device_ip: Ipv4Addr,
}

impl InterfaceData {
    pub fn fetch_from_interface(interface_name: &str) -> InterfaceData {
        let iface = pnet_datalink::interfaces()
            .into_iter()
            .find(|interface| interface.name == interface_name)
            .expect("Could not find specified interface");

        let gateway_ip = Self::fetch_gateway_ip(&iface.name)
            .expect("Interface data fetching failure: could not fetch gateway ip");

        let gateway_mac = Self::get_mac_from_ip(gateway_ip)
            .expect("Interface data fetching failure: could not fetch the gateway mac");

        let device_ip = iface
            .ips
            .iter()
            .find(|ip| ip.is_ipv4())
            .map(|ip| ip.to_owned())
            .expect("Interface data fetching failure: ip not found");

        let device_ip = match device_ip {
            IpNetwork::V4(ip) => ip.ip(),
            IpNetwork::V6(_) => panic!("IPV6 is not supported"),
        };

        Self {
            iface,
            gateway_mac,
            gateway_ip,
            device_ip,
        }
    }

    pub fn fetch_default() -> InterfaceData {
        let iface = Self::fetch_default_interface().unwrap();

        let gateway_ip = Self::fetch_gateway_ip(&iface.name).unwrap();

        let gateway_mac = Self::get_mac_from_ip(gateway_ip).unwrap();

        let device_ip = iface
            .ips
            .iter()
            .find(|ip| ip.is_ipv4())
            .map(|ip| ip.to_owned())
            .unwrap();

        let device_ip = match device_ip {
            IpNetwork::V4(ip) => ip.ip(),
            IpNetwork::V6(_) => panic!("IPV6 is not supported"),
        };

        Self {
            iface,
            gateway_mac,
            gateway_ip,
            device_ip,
        }
    }

    fn get_mac_from_ip(ip: Ipv4Addr) -> Result<MacAddr, ()> {
        let arp_info = fs::read_to_string("/proc/net/arp").map_err(|_| ())?;

        for line in arp_info.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts[0] == ip.to_string() {
                let mac = MacAddr::from_str(parts[3]).unwrap();
                return Ok(mac);
            }
        }

        Err(())
    }

    fn fetch_gateway_ip(interface_name: &str) -> Option<Ipv4Addr> {
        let route_info = fs::read_to_string("/proc/net/route").ok()?;

        for line in route_info.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts[0] == interface_name && parts[1] == "00000000" && parts[7] == "00000000" {
                let ip = u32::from_str_radix(parts[2], 16).ok()?;

                let ip = ((ip & 0x000000FF) << 24)
                    | ((ip & 0x0000FF00) << 8)
                    | ((ip & 0x00FF0000) >> 8)
                    | ((ip & 0xFF000000) >> 24);

                return Some(Ipv4Addr::from(ip));
            }
        }

        None
    }

    fn fetch_default_interface_from_proc() -> Option<pnet_datalink::NetworkInterface> {
        let route_info = fs::read_to_string("/proc/net/route").ok()?;

        for line in route_info.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts[1] == "00000000" && parts[7] == "00000000" {
                return pnet_datalink::interfaces()
                    .into_iter()
                    .find(|i| i.name == parts[0]);
            }
        }
        None
    }

    fn fetch_default_interface_from_pnet() -> Option<pnet_datalink::NetworkInterface> {
        pnet_datalink::interfaces()
            .into_iter()
            .find(|i| i.is_up() && !i.is_loopback() && !i.name.is_empty())
    }

    fn fetch_default_interface() -> Option<pnet_datalink::NetworkInterface> {
        Self::fetch_default_interface_from_proc().or_else(Self::fetch_default_interface_from_pnet)
    }
}
