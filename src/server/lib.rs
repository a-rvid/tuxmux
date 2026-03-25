use std::process::{ChildStdin, Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::io::AsRawFd;
use std::thread;

pub struct Session {
    stdin: ChildStdin,
    stdout_receiver: mpsc::Receiver<String>,
}

type Sessions = Arc<Mutex<Vec<Session>>>;
type Clients = Arc<Mutex<Vec<UnixStream>>>;

impl Session {
    pub fn spawn() -> Session {
        let mut child = Command::new("/bin/bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn shell");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx.send(line);
                }
            }
        });

        Session { stdin, stdout_receiver: rx }
    }
}

fn handle_connection(stream: UnixStream, sessions: Sessions, clients: Clients, active_session: Arc<Mutex<usize>>) { 
    {
        let mut cls = clients.lock().unwrap();
        cls.push(stream.try_clone().unwrap());
    }

    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // disconnected
            Ok(_) => {
                let sessions_guard = sessions.lock().unwrap();
                let idx = *active_session.lock().unwrap();
                if let Some(session) = sessions_guard.get(idx) {
                    let _ = writeln!(&session.stdin, "{}", line);
                }
            }
            Err(_) => break,
        }
    }

    // Remove client from list when disconnected
    let mut cls = clients.lock().unwrap();
    cls.retain(|c| c.as_raw_fd() != stream.as_raw_fd());
}

// Broadcast lines from sessions to all clients
fn spawn_output_forwarder(sessions: Sessions, clients: Clients) {
    thread::spawn(move || loop {
        let sessions_guard = sessions.lock().unwrap();
        for session in sessions_guard.iter() {
            while let Ok(line) = session.stdout_receiver.try_recv() {
                let cls = clients.lock().unwrap();
                for mut client in cls.iter() {
                    let _ = writeln!(client, "{}", line);
                }
            }
        }
        drop(sessions_guard);
        thread::sleep(std::time::Duration::from_millis(10));
    });
}

pub fn server(socket_path: &str) {
    let listener = UnixListener::bind(socket_path).unwrap();
    let sessions: Sessions = Arc::new(Mutex::new(vec![Session::spawn()]));
    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let active_session = Arc::new(Mutex::new(0));

    spawn_output_forwarder(sessions.clone(), clients.clone());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let s_clone = sessions.clone();
                let c_clone = clients.clone();
                let a_clone = active_session.clone();
                thread::spawn(move || handle_connection(stream, s_clone, c_clone, a_clone));
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
}