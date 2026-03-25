use std::env;
mod lib;

fn main() {
    let socket: String = env::var("TUXMUX_SOCKET").unwrap_or("/tmp/tuxmux.sock".to_string());
    lib::server(&socket);
}