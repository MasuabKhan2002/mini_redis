use std::net::{TcpListener, TcpStream};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

type Store = Arc<Mutex<HashMap<String, String>>>;

/// Processes incoming client commands and returns a response
fn match_command(command: &str, store: &Store) -> String {
    let mut parts = command.splitn(3, " ");
    let cmd = parts.next();

    match cmd {
        Some("SET") => {
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let mut db = store.lock().unwrap();
                db.insert(key.to_string(), value.to_string());
                "OK".to_string()
            } else {
                "ERROR: Invalid SET command".to_string()
            }
        }
        Some("GET") => {
            if let Some(key) = parts.next() {
                let db = store.lock().unwrap();
                db.get(key).cloned().unwrap_or("NULL".to_string())
            } else {
                "ERROR: Invalid GET command".to_string()
            }
        }
        _ => "ERROR: Unknown command".to_string(),
    }
}

/// Handles a client connection and processes commands
fn handle_client(stream: TcpStream, store: Store) {
    let reader = BufReader::new(stream.try_clone().expect("Failed to clone stream"));
    let mut writer = stream;

    for line in reader.lines() {
        match line {
            Ok(msg) => {
                println!("Received: {}", msg);
                let response = match_command(&msg, &store);
                writer.write_all(format!("{}\n", response).as_bytes()).unwrap();
            }
            Err(_) => {
                println!("Client disconnected.");
                break;
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").expect("Failed to bind to port");
    let store: Store = Arc::new(Mutex::new(HashMap::new()));

    println!("Listening on 127.0.0.1:6379...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let store = Arc::clone(&store);
                thread::spawn(move || handle_client(stream, store));
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
}