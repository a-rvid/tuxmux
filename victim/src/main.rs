#![no_std]
#![no_main]

#[cfg(debug_assertions)]
mod debug;

#[cfg(debug_assertions)]
use crate::debug::print;

mod hex;

extern crate alloc;

use talc::{*, source::Claim}; // I have not researched much about this allocator, might switch later.
use sha2::{Digest, Sha256};
use spinning_top::RawSpinlock;

#[global_allocator]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(unsafe {
    static mut INITIAL_HEAP: [u8; min_first_heap_size::<DefaultBinning>() + 2000] =
        [0; min_first_heap_size::<DefaultBinning>() + 2000];

    Claim::array(&raw mut INITIAL_HEAP)
});

#[cfg(all(not(panic = "immediate-abort"), not(test)))]
use core::panic::PanicInfo;

#[cfg(all(not(panic = "immediate-abort"), not(test)))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}

#[unsafe(no_mangle)]
extern "C" fn main() -> u8 {
    0
}

#[cfg(test)]
mod tests {
    use crate::hex;
    use sha2::{Digest, Sha256};

    const STRING: &str = "test";

    #[test]
    fn test_hex_sha256() { // tests sha256 and hex encoding
        assert_eq!("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08", hex::encode(Sha256::digest(STRING.as_bytes())));
    }
}