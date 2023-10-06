use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use aes::Aes128;

use rand::{self, Rng};
use std::net::Ipv4Addr;
use std::ops::RangeInclusive;

#[derive(Clone, Debug)]
pub struct CookieHasher {
    cipher: Aes128,
    ephemeral_range: RangeInclusive<u16>,
}

impl CookieHasher {
    pub fn new() -> Self {
        let mut secret: Vec<u8> = vec![];
        let mut rng = rand::thread_rng();
        for _i in 0..16 {
            secret.push(rng.gen())
        }
        let key = GenericArray::clone_from_slice(&secret[..]);
        let cipher = Aes128::new(&key);
        let ephemeral_range = Self::fetch_ephemeral_range_from_proc();
        Self {
            cipher,
            ephemeral_range,
        }
    }

    pub fn get_port_cookie(&self, src_ip: Ipv4Addr, dest_ip: Ipv4Addr) -> u16 {
        let mut buf = [0u8; 16];
        buf[..4].copy_from_slice(&src_ip.octets());
        buf[4..8].copy_from_slice(&dest_ip.octets());
        buf[8..16].copy_from_slice(&[0u8; 8]);
        let mut data = GenericArray::clone_from_slice(&mut buf);
        self.cipher.encrypt_block(&mut data);
        let hash = u16::from_be_bytes([data[0], data[15]]);
        return (hash % self.ephemeral_range.len() as u16) + self.ephemeral_range.start();
    }

    pub fn check_port_cookie(&self, ip: Ipv4Addr, dst_ip: Ipv4Addr, dst_port: u16) -> bool {
        let hash = self.get_port_cookie(ip, dst_ip);
        return hash == dst_port;
    }

    fn fetch_ephemeral_range_from_proc() -> RangeInclusive<u16> {
        let range_data = std::fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range").unwrap();
        let range_data: Vec<&str> = range_data.split_whitespace().collect();
        let start_ephemeral = range_data[0].parse::<u16>().unwrap();
        let end_ephemeral = range_data[1].parse::<u16>().unwrap();
        start_ephemeral..=end_ephemeral
    }
}
