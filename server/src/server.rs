use dns_server::DnsRecord;
use permit::Permit;
use x25519_dalek::{StaticSecret, SharedSecret, PublicKey};
use rusqlite::Connection;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::path::Path;
use tokio::fs;
use users::get_current_uid;

/// Config file
const DATA: &str = "/etc/tuxmux";

const SPLASH: &str = r" /_  __/_  ___  __/  |/  /_  ___  __
  / / / / / / |/_/ /|_/ / / / / |/_/
 / / / /_/ />  </ /  / / /_/ />  <  
/_/  \__,_/_/|_/_/  /_/\__,_/_/|_|";

struct Keypair {
    private: StaticSecret,
    public: PublicKey,
}

impl Keypair {
    fn generate() -> Self {
        let private = StaticSecret::random();
        let public = PublicKey::from(&private);
        Self {
            private,
            public,
        }
    }
}

// fn generate_keypair() -> (StaticSecret, PublicKey) {
//     let mut csprng = CryptoRng;
//     let secret = StaticSecret::new(&mut csprng);
//     let public = PublicKey::from(&secret);
//     (secret, public)
// }

#[tokio::main]
async fn main() {
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
        Keypair { private, public }
    } else {
        let keypair = Keypair::generate();
        fs::write(data.join("private.key"), &keypair.private.to_bytes()).await.unwrap();
        fs::write(data.join("public.key"), &keypair.public.to_bytes()).await.unwrap();
        keypair
    };
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

        CREATE INDEX IF NOT EXISTS idx_status_client   ON status(client_id);
        CREATE INDEX IF NOT EXISTS idx_commands_client ON commands(client_id);
        CREATE INDEX IF NOT EXISTS idx_commands_status ON commands(status);
        CREATE INDEX IF NOT EXISTS idx_operators_key   ON operators(auth_key);

        COMMIT;
    ",
    )?;
    Ok(conn)
}
