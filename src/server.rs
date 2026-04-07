use dns_server::DnsRecord;
use permit::Permit;
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs8::EncodePrivateKey, pkcs8::EncodePublicKey};
use rusqlite::Connection;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tokio::fs;
use users::get_current_uid;

/// RSA private key
fn generate_keypair(keysize: usize) -> (RsaPrivateKey, RsaPublicKey) {
    let mut rng = rsa::rand_core::OsRng;
    let keypair = RsaPrivateKey::new(&mut rng, keysize).unwrap();
    let public_key = keypair.to_public_key();
    (keypair, public_key)
}

/// Config file
const DATA: &str = "/etc/tuxmux";

const SPLASH: &str = r"/_  __/_  ___  __/  |/  /_  ___  __
  / / / / / / |/_/ /|_/ / / / / |/_/
 / / / /_/ />  </ /  / / /_/ />  <  
/_/  \__,_/_/|_/_/  /_/\__,_/_/|_|";

#[tokio::main]
async fn main() {
    let data = if let Ok(config) = std::env::var("TUXMUX_CONFIG") {
        config
    } else if get_current_uid() == 0 {
        DATA.to_string()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{home}/.tuxmux")
    };

    let port: u16 = std::env::var("TUXMUX_PORT")
        .unwrap_or_else(|_| "53".to_string())
        .parse()
        .unwrap();

    let data = Path::new(&data);
    if !data.exists() {
        fs::create_dir(data).await.unwrap();
    }

    let conn = init_db(data).await.unwrap(); 

    if !fs::try_exists(data.join("private.der")).await.unwrap() {
        let (private_key, public_key) = generate_keypair(2048);
        let private_key_file = private_key.to_pkcs8_der().unwrap();
        let public_key_file = public_key.to_public_key_der().unwrap();

        fs::write(
            data.join("private.der"),
            private_key_file.as_bytes().as_ref(),
        )
        .await
        .unwrap();
        fs::write(data.join("public.der"), public_key_file.as_ref())
            .await
            .unwrap();
        fs::set_permissions(data.join("public.der"), Permissions::from_mode(0o600))
            .await
            .unwrap();
        fs::set_permissions(data.join("private.der"), Permissions::from_mode(0o600))
            .await
            .unwrap();
    }

    let top_permit = Permit::new();
    let permit = top_permit.new_sub();

    std::thread::spawn(move || {
        Signals::new([SIGINT, SIGTERM])
            .unwrap()
            .forever()
            .next()
            .unwrap();
        drop(top_permit);
    });

    let records = vec![
        DnsRecord::new_txt("example.com", "Hello, world!").unwrap(),
        DnsRecord::new_aaaa("example.com", "::1").unwrap(),
        DnsRecord::new_a("example.com", "0.0.0.0").unwrap(),
        DnsRecord::new_cname("cname.example.com", "example.com").unwrap(),
    ];

    println!("{}", SPLASH); // TUXMUX splash
    println!("TuxMux, Config directory: {}", data.display());
    println!("Starting DNS server on port {}", port);

    dns_server::Builder::new_port(port)
        .unwrap()
        .with_permit(permit)
        .serve_static(&records)
        .unwrap();
}

async fn init_db(path: &Path) -> rusqlite::Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path.join("tuxmux.db"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;


    // clients
    conn.execute(
        "CREATE TABLE IF NOT EXISTS clients (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT,
            description TEXT,
            public_key  BLOB    NOT NULL,
            private_key BLOB    NOT NULL
        )",
        [],
    )?;

    // status
    // heartbeat: epoch!
    // privilege: user permission level
    conn.execute(
        "CREATE TABLE IF NOT EXISTS status (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            privilege   INTEGER,
            client_id   INTEGER NOT NULL,
            heartbeat   INTEGER NOT NULL,
            status      INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // commands
    conn.execute(
        "CREATE TABLE IF NOT EXISTS commands (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            client_id     INTEGER NOT NULL,
            session_id    TEXT    NOT NULL,
            command_queue TEXT    NOT NULL DEFAULT '[]', -- json
            FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // --- operators ---
    // Authenticated operator sessions. auth_key should be a hashed token,
    // never stored in plaintext. current_client_id is nullable (no active session).
    conn.execute(
        "CREATE TABLE IF NOT EXISTS operators (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            auth_key          TEXT    NOT NULL UNIQUE, 
            name              TEXT    NOT NULL,
            current_client_id INTEGER,
            FOREIGN KEY (current_client_id) REFERENCES clients(id) ON DELETE SET NULL
        )",
        [],
    )?;

    // indexes
    // Speed up the most common lookups (client_id filters)
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_status_client   ON status(client_id);
         CREATE INDEX IF NOT EXISTS idx_commands_client ON commands(client_id);
         CREATE INDEX IF NOT EXISTS idx_commands_status ON commands(status);
         CREATE INDEX IF NOT EXISTS idx_operators_key   ON operators(auth_key);",
    )?;
    Ok(conn)
}