use rand::Rng;

use std::intrinsics::{wrapping_add, wrapping_mul};
use std::net::{Ipv4Addr, SocketAddrV4};

pub struct Lcg {
    pub state: u32,
    pub a: u32,
    pub c: u32,
}

impl Iterator for Lcg {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let num = self.rand();
        if num == 0 {
            return None;
        };
        Some(num)
    }
}

impl Lcg {
    pub fn rand(&mut self) -> u32 {
        self.state = wrapping_add(wrapping_mul(self.state, self.a), self.c);
        self.state
    }
}

impl Default for Lcg {
    fn default() -> Self {
        let rng: u32 = rand::thread_rng().gen_range(0..1000);

        Self {
            state: 1,
            a: (rng * 4 + 1),
            c: 1,
        }
    }
}

pub struct IPv4Iterator {
    port: u16,
    lcg: Lcg,
}

impl IPv4Iterator {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            lcg: Lcg::default(),
        }
    }
}

impl Iterator for IPv4Iterator {
    type Item = SocketAddrV4;

    fn next(&mut self) -> Option<Self::Item> {
        let ip = match self.lcg.next() {
            Some(ip) => ip,
            None => return None,
        };
        let addr = Some(SocketAddrV4::new(Ipv4Addr::from_bits(ip), self.port));
        addr
    }
}
