use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::fs;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json;
use dotenv::dotenv;
use std::env;

type Store = Arc<Mutex<HashMap<String, String>>>;

fn get_db_file_path() -> String {
    dotenv().ok();
    env::var("DB_FILE").unwrap_or_else(|_| "db.json".to_string())
}

async fn save_to_file(store: &Store) {
    let db_file = get_db_file_path();
    let db = store.lock().await;
    let json = serde_json::to_string(&*db).unwrap();
    fs::write(db_file, json).await.unwrap();
}

async fn load_from_file() -> HashMap<String, String> {
    let db_file = get_db_file_path();

    match fs::read_to_string(&db_file).await {
        Ok(content) => {
            match serde_json::from_str(&content) {
                Ok(data) => data,
                Err(_) => {
                    println!("Warning: Corrupted database file. Starting fresh.");
                    HashMap::new()
                }
            }
        }
        Err(_) => {
            println!("No database file found at '{}'. Starting fresh.", db_file);
            HashMap::new()
        }
    }
}

async fn match_command(command: &str, store: &Store) -> String {
    let mut parts = command.trim().splitn(3, " ");
    let cmd = parts.next();

    match cmd {
        Some("SET") => {
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let mut db = store.lock().await;
                db.insert(key.to_string(), value.to_string());
                drop(db);

                save_to_file(store).await;

                "OK".to_string()
            } else {
                "ERROR: Usage: SET <key> <value>".to_string()
            }
        }
        Some("GET") => {
            if let Some(key) = parts.next() {
                let db = store.lock().await;
                db.get(key).cloned().unwrap_or("NULL".to_string())
            } else {
                "ERROR: Usage: GET <key>".to_string()
            }
        }
        Some("") | None => "ERROR: Empty command".to_string(),
        _ => "ERROR: Unknown command. Use SET <key> <value> or GET <key>".to_string(),
    }
}

async fn handle_client(stream: TcpStream, store: Store) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader).lines();

    writer.write_all(b"> ").await.unwrap();

    while let Ok(Some(line)) = reader.next_line().await {
        println!("Received: {}", line);
        let response = match_command(&line, &store).await;
        
        writer.write_all(format!("{}\n> ", response).as_bytes()).await.unwrap();
    }

    println!("Client disconnected.");
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    let store: Store = Arc::new(Mutex::new(load_from_file().await));

    println!("Listening on 127.0.0.1:6379...");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            handle_client(stream, store).await;
        });
    }
}