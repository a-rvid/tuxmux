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
}

type TcpWriters = Arc<Mutex<Vec<tokio::net::tcp::OwnedWriteHalf>>>;
type UnixWriters = Arc<Mutex<Vec<tokio::net::unix::OwnedWriteHalf>>>;

/// Server to operator
const MOTD_TYPE: u8 = 0x01;   // '1' — server MOTD sent once on connect
const NOTIFICATION: u8 = 0x02; // '2' — transient notification

/// Operator to server
const CMD_NOTIFY: u8 = 0x01; // notification command

/// MOTD
const MOTD: &str = "Welcome to TuxMux!\\nhttps://github.com/a-rvid/tuxmux/\\n\\ntype  :h | :help<ENTER>      if you are new         \\ntype  :q | :quit<ENTER>      to exit                 \\ntype  :a | :all<ENTER>       to send to all clients \\ntype  i<ENTER>               to enter insert mode    \\ntype  Escape<ENTER>          to enter normal mode    \n";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let tcp_writers: TcpWriters = Arc::new(Mutex::new(Vec::new()));
    let unix_writers: UnixWriters = Arc::new(Mutex::new(Vec::new()));

    // Remove existing UNIX socket
    if Path::new(&args.socket).exists() {
        std::fs::remove_file(&args.socket)?;
    }

    /// --- TCP listener (reverse shells connect here) ---
    // let tcp_listener = TcpListener::bind(format!("{}:{}", args.address, args.port)).await?;
    // let tcp_writers_clone = tcp_writers.clone();
    // let unix_writers_clone = unix_writers.clone();

    // tokio::spawn(async move {
    //     loop {
    //         match tcp_listener.accept().await {
    //             Ok((stream, addr)) => {
    //                 println!("Shell connected: {}", addr);
    //                 let (mut reader, writer) = stream.into_split();
    //                 tcp_writers_clone.lock().await.push(writer);

    //                 let unix_writers = unix_writers_clone.clone();

    //                 tokio::spawn(async move {
    //                     let mut buf = [0u8; 4096];
    //                     loop {
    //                         match reader.read(&mut buf).await {
    //                             Ok(0) => break,
    //                             Ok(n) => {
    //                                 let mut u = unix_writers.lock().await;
    //                                 let mut i = 0;
    //                                 while i < u.len() {
    //                                     if u[i].write_all(&buf[..n]).await.is_err() {
    //                                         u.swap_remove(i);
    //                                     } else {
    //                                         i += 1;
    //                                     }
    //                                 }
    //                             }
    //                             Err(_) => break,
    //                         }
    //                     }
    //                     println!("Shell disconnected");
    //                 });
    //             }
    //             Err(e) => eprintln!("TCP accept error: {}", e),
    //         }
    //     }
    // });

    /// --- UNIX listener (operators connect here) ---
    let unix_listener = UnixListener::bind(&args.socket)?;
    println!("Listening: unix://{}", args.socket);
    let tcp_writers_clone = tcp_writers.clone();
    let unix_writers_clone = unix_writers.clone();

    tokio::spawn(async move {
        loop {
            match unix_listener.accept().await {
                Ok((stream, _)) => {
                    println!("Operator connected");
                    let (mut reader, mut writer) = stream.into_split();

                    let _ = writer
                        .write_all(format!("{NOTIFICATION}Operator connected\n").as_bytes())
                        .await;

                    let _ = writer.write_all(format!("{MOTD_TYPE}{MOTD}").as_bytes()).await;

                    writer.flush().await.unwrap();
                    unix_writers_clone.lock().await.push(writer);

                    let tcp_writers = tcp_writers_clone.clone();

                    let unix_writers = unix_writers_clone.clone();

                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        loop {
                            match reader.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    if buf[0] == CMD_NOTIFY {
                                        // Handle notification command
                                        let msg = String::from_utf8_lossy(&buf[1..n]).trim_end_matches('\n').to_string();
                                        let notification = format!("{}{}\n", NOTIFICATION, msg);
                                        println!("NOTIFY: {}", msg);

                                        // Broadcast notification to all connected operators
                                        let mut u = unix_writers.lock().await;
                                        let mut i = 0;
                                        while i < u.len() {
                                            if u[i].write_all(notification.as_bytes()).await.is_err() {
                                                u.swap_remove(i);
                                            } else {
                                                if u[i].flush().await.is_err() {
                                                    u.swap_remove(i);
                                                } else {
                                                    i += 1;
                                                }
                                            }
                                        }
                                    } else {
                                        // Regular output to TCP writers
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

    // Connect to remote bind server
    // let tcp_writers_clone = tcp_writers.clone();
    // let bind_target = args.bind.clone();
    // tokio::spawn(async move {
    //     match TcpStream::connect(&bind_target).await {
    //         Ok(stream) => {
    //             println!("Connected to bind server {}", bind_target);
    //             let (_r, w) = stream.into_split();
    //             tcp_writers_clone.lock().await.push(w);
    //         }
    //         Err(e) => eprintln!("Failed to connect to bind server {}: {}", bind_target, e),
    //     }
    // });

    // println!("Listening: TCP {} UNIX {}", args.socket);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
