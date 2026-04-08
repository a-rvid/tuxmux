#![feature(allocator_api)]
#![no_std]
#![no_main]

#[cfg(debug_assertions)]
mod debug;

#[cfg(debug_assertions)]
use crate::debug::print;

mod hex;

extern crate alloc;

use talc::{*, source::Claim};
use sha2::{Digest, Sha256};
use spinning_top::RawSpinlock;

#[global_allocator]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(unsafe {
    static mut INITIAL_HEAP: [u8; min_first_heap_size::<DefaultBinning>() + 100000] =
        [0; min_first_heap_size::<DefaultBinning>() + 100000];

    Claim::array(&raw mut INITIAL_HEAP)
});

#[cfg(not(panic = "immediate-abort"))]
use core::panic::PanicInfo;

#[cfg(not(panic = "immediate-abort"))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}

#[unsafe(no_mangle)]
extern "C" fn main() -> u8 {
    #[cfg(debug_assertions)]
    {
        let string: &str = "test";
        let hex = hex::encode(string.as_bytes());
        println!("sha256 test: {:?}", hex::encode(Sha256::digest(string.as_bytes())));
        println!("hex test: {:?}", hex);
        println!("Tuxmux client, do not use in production\n{}", string);
        for i in 0..5 {
            println!("int loop {}", i);
        }
    }
    0
}