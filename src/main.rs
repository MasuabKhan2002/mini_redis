use dotenv::dotenv;
use serde_json;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};

type Sender = broadcast::Sender<String>;

// Struct to represent a subscription with multiple subscribers
struct Subscription {
    sender: Sender,
    subscribers: HashMap<u64, mpsc::UnboundedSender<String>>,
}

type Subscriptions = Arc<Mutex<HashMap<String, Subscription>>>;
type Store = Arc<Mutex<HashMap<String, (String, Option<u64>)>>>;

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn cleanup_expired_keys(store: Store) {
    loop {
        sleep(Duration::from_secs(10)).await;

        let mut db = store.lock().await;
        let now = current_timestamp();

        db.retain(|_, (_, expires_at)| {
            if let Some(expiry) = expires_at {
                now < *expiry
            } else {
                true
            }
        });
    }
}

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

async fn load_from_file() -> HashMap<String, (String, Option<u64>)> {
    let db_file = get_db_file_path();

    match fs::read_to_string(&db_file).await {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(data) => data,
            Err(_) => {
                println!("Warning: Corrupted database file. Starting fresh.");
                HashMap::new()
            }
        },
        Err(_) => {
            println!("No database file found at '{}'. Starting fresh.", db_file);
            HashMap::new()
        }
    }
}

async fn match_command(
    command: &str,
    store: &Store,
    subscriptions: &Subscriptions,
    writer: Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    client_id: u64,
) -> String {
    let mut parts = command.trim().splitn(3, " ");
    let cmd = parts.next();
    match cmd {
        Some("SUBSCRIBE") => {
            if let Some(channel) = parts.next() {
                let mut subs = subscriptions.lock().await;
                let (client_sender, mut client_receiver) = mpsc::unbounded_channel::<String>();

                // If channel exists, add the subscriber
                let subscription = subs.entry(channel.to_string()).or_insert_with(|| {
                    let (sender, mut broadcast_receiver) = broadcast::channel::<String>(100);
                    let subs_clone = Arc::clone(&subscriptions);
                    let channel_clone = channel.to_string();

                    // Spawn one task per channel to broadcast messages
                    tokio::spawn(async move {
                        while let Ok(message) = broadcast_receiver.recv().await {
                            let subs = subs_clone.lock().await;
                            if let Some(sub) = subs.get(&channel_clone) {
                                for subscriber in sub.subscribers.values() {
                                    let _ = subscriber.send(message.clone());
                                }
                            }
                        }
                    });

                    Subscription {
                        sender,
                        subscribers: HashMap::new(),
                    }
                });

                // Add subscriber
                subscription.subscribers.insert(client_id, client_sender);
                let channel_clone = channel.to_string();

                // Spawn lightweight task to listen for messages and write to client
                let writer_clone = Arc::clone(&writer);
                tokio::spawn(async move {
                    while let Some(message) = client_receiver.recv().await {
                        let mut writer_lock = writer_clone.lock().await;
                        writer_lock
                            .write_all(
                                format!("CHANNEL: {}, MESSAGE: {}\n> ", channel_clone, message)
                                    .as_bytes(),
                            )
                            .await
                            .unwrap();
                    }
                });

                "OK".to_string()
            } else {
                "ERROR: Usage: SUBSCRIBE <channel>".to_string()
            }
        }
        Some("PUBLISH") => {
            if let (Some(channel), Some(message)) = (parts.next(), parts.next()) {
                let subs = subscriptions.lock().await;
                if let Some(sub) = subs.get(channel) {
                    let _ = sub.sender.send(message.to_string());
                    "OK".to_string()
                } else {
                    "ERROR: No subscribers found for the channel.".to_string()
                }
            } else {
                "ERROR: Usage: PUBLISH <channel> <message>".to_string()
            }
        }
        Some("UNSUBSCRIBE") => {
            if let Some(channel) = parts.next() {
                let mut subs = subscriptions.lock().await;
                if let Some(sub) = subs.get_mut(channel) {
                    sub.subscribers.remove(&client_id);
                    if sub.subscribers.is_empty() {
                        subs.remove(channel);
                    }
                    "OK".to_string()
                } else {
                    "ERROR: No subscribers found for the channel.".to_string()
                }
            } else {
                "ERROR: Usage: UNSUBSCRIBE <channel>".to_string()
            }
        }
        Some("SET") => {
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let mut db = store.lock().await;
                db.insert(key.to_string(), (value.to_string(), None));
                drop(db);

                save_to_file(store).await;

                "OK".to_string()
            } else {
                "ERROR: Usage: SET <key> <value>".to_string()
            }
        }
        Some("GET") => {
            if let Some(key) = parts.next() {
                let mut db = store.lock().await;
                if let Some((value, expires_at)) = db.get(key) {
                    if let Some(expiry) = expires_at {
                        if current_timestamp() > *expiry {
                            db.remove(key); // Key has expired, remove it
                            drop(db);
                            save_to_file(store).await;
                            return "NULL".to_string();
                        }
                    }
                    return value.clone();
                }
                "NULL".to_string()
            } else {
                "ERROR: Usage: GET <key>".to_string()
            }
        }
        Some("DEL") => {
            if let Some(key) = parts.next() {
                let mut db = store.lock().await;
                db.remove(key);
                drop(db);

                save_to_file(store).await;

                "OK".to_string()
            } else {
                "ERROR: Usage: DEL <key>".to_string()
            }
        }
        Some("EXPIRE") => {
            if let (Some(key), Some(seconds)) = (parts.next(), parts.next()) {
                if let Ok(seconds) = seconds.parse::<u64>() {
                    let mut db = store.lock().await;
                    if let Some((value, _)) = db.get(key).cloned() {
                        db.insert(
                            key.to_string(),
                            (value, Some(current_timestamp() + seconds)),
                        );
                        drop(db);

                        save_to_file(store).await;

                        return "OK".to_string();
                    } else {
                        return "NULL".to_string();
                    }
                } else {
                    return "ERROR: Invalid expiration time".to_string();
                }
            } else {
                "ERROR: Usage: EXPIRE <key> <seconds>".to_string()
            }
        }
        Some("") | None => "ERROR: Empty command".to_string(),
        _ => "ERROR: Unknown command. Use SET, GET, DEL, or EXPIRE.".to_string(),
    }
}

async fn handle_client(
    stream: TcpStream,
    store: Store,
    subscriptions: Subscriptions,
    client_id: u64,
) {
    println!("Client {} connected. ", client_id);
    let (reader, writer) = stream.into_split();
    let writer = Arc::new(Mutex::new(writer));
    let mut reader = BufReader::new(reader).lines();

    {
        let mut writer_lock = writer.lock().await;
        writer_lock.write_all(b"> ").await.unwrap();
    }

    while let Ok(Some(line)) = reader.next_line().await {
        println!("Received: {}", line);
        let response = match_command(
            &line,
            &store,
            &subscriptions,
            Arc::clone(&writer),
            client_id,
        )
        .await;

        {
            let mut writer_lock = writer.lock().await;
            writer_lock
                .write_all(format!("{}\n> ", response).as_bytes())
                .await
                .unwrap();
        }

        if response == "GOODBYE" {
            println!("Client disconnected.");
            break;
        }
    }

    println!("Client disconnected.");
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    let store: Store = Arc::new(Mutex::new(load_from_file().await));
    let subscriptions: Subscriptions = Arc::new(Mutex::new(HashMap::new()));
    let client_counter = Arc::new(AtomicU64::new(1));

    // Spawn background task for cleaning up expired keys
    let store_clone = Arc::clone(&store);
    tokio::spawn(async move {
        cleanup_expired_keys(store_clone).await;
    });

    println!("Listening on 127.0.0.1:6379...");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let store = Arc::clone(&store);
        let client_id = client_counter.fetch_add(1, Ordering::Relaxed);
        let subscriptions = Arc::clone(&subscriptions);
        tokio::spawn(async move {
            handle_client(stream, store, subscriptions, client_id).await;
        });
    }
}
