use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Store = Arc<Mutex<HashMap<String, String>>>;

async fn match_command(command: &str, store: &Store) -> String {
    let mut parts = command.splitn(3, " ");
    let cmd = parts.next();

    match cmd {
        Some("SET") => {
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let mut db = store.lock().await;
                db.insert(key.to_string(), value.to_string());
                "OK".to_string()
            } else {
                "ERROR: Invalid SET command".to_string()
            }
        }
        Some("GET") => {
            if let Some(key) = parts.next() {
                let db = store.lock().await;
                db.get(key).cloned().unwrap_or("NULL".to_string())
            } else {
                "ERROR: Invalid GET command".to_string()
            }
        }
        _ => "ERROR: Unknown command".to_string(),
    }
}

async fn handle_client(stream: TcpStream, store: Store) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        println!("Received: {}", line);
        let response = match_command(&line, &store).await;
        writer.write_all(format!("{}\n", response).as_bytes()).await.unwrap();
    }

    println!("Client disconnected.");
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    let store: Store = Arc::new(Mutex::new(HashMap::new()));

    println!("Listening on 127.0.0.1:6379...");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            handle_client(stream, store).await;
        });
    }
}