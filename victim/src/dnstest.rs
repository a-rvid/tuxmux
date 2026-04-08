#![no_std]

use nostd::{
    fs::File,
    io::Read,
    vec::Vec,
    string::String,
    prelude::*
};
use no_std_net::{SocketAddr, UdpSocket};

use talc::{*, source::Claim}; // I have not researched much about this allocator, might switch later.
use spinning_top::RawSpinlock;
mod dns;
mod debug;
use crate::debug::print;

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

fn main() -> () {
    let args: Vec<String> = vec!["example.com".to_string(), "low-effort.work".to_string()];
    if args.len() < 2 {
        show_help();
        return ();
    }
    let domain_name = &args[1];
    let domains = vec![domain_name.clone()];
    // TODO: server's IP address should be passed via cli argument
    let dns_server_address = [8, 8, 8, 8];
    let server = SocketAddr::from((dns_server_address, 53));
    let client = SocketAddr::from(([0, 0, 0, 0], 0));
    let sock = UdpSocket::bind(client).map_err(|err| err.to_string()).unwrap();
    let query = dns::Query::new(67, &domains);

    // // send UDP packet to the server
    let buf: Vec<u8> = query.into();
    sock.send_to(&buf, server).map_err(|err| err.to_string()).unwrap();
    let mut buf = [0; 512];
    sock.recv_from(&mut buf).map_err(|err| err.to_string()).unwrap();
    let response = dns::Response::try_from(&buf).unwrap();

    println!(
        "====\nIP address for {} is {}",
        domain_name,
        response.answers[0].rdata.to_string(),
    );

    ();
}

fn show_help() {
    println!(
        "
USAGE:
$ dns <domain_name_you_want_to_resolve>
    "
    )
}

fn get_random_u16() -> u16 {
    // TODO: this should not work on Windows
    let mut file = File::open("/dev/urandom").unwrap();
    let mut buffer = [0u8; 2];
    file.read_exact(&mut buffer).unwrap();
    ((buffer[0] as u16) << 8) + (buffer[1] as u16)
}