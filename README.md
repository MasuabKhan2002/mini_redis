# 📝 Project Plan: Mini Redis (Key-Value Store)

## 🔹 Step 1: Define Project Scope & Features
We aim to build a simple in-memory key-value store with basic Redis-like behavior. The core functionality will include:

- **TCP Server**: Clients can connect via TCP.
- **Command Handling**: Support for `SET key value` and `GET key`.
- **Concurrency**: Handle multiple client connections.
- **Data Storage**: In-memory `HashMap` for storing key-value pairs.
- **Persistence (optional)**: Save/load data from disk.

---

## 🔹 Step 2: Define High-Level Architecture

### 1️⃣ Core Components

### **TCP Server**
- Listens for incoming connections.
- Spawns a handler for each client.

### **Command Parser**
- Reads commands from clients.
- Parses input (e.g., `SET foo bar` → `key=foo, value=bar`).

### **Storage Layer**
- Uses `HashMap<String, String>` as an in-memory store.
- Uses `Arc<Mutex<HashMap<..>>>` to share the store across threads.

### **Concurrency Handling**
- **Threaded version**: Each connection is handled in a separate thread.
- **Async version**: Uses `tokio` for efficient async handling.

---

## 🔹 Step 3: Plan the Data Flow

### **Client-Server Interaction**
1. **Client sends a command over TCP**, e.g., `"SET foo bar"`.
2. **Server receives the command**, parses it.
3. **Server updates or retrieves data** from the `HashMap`.
4. **Server responds back** with `"OK"` or the stored value.

---

## 🔹 Step 4: Define Commands

| Command             | Functionality                           |
|---------------------|--------------------------------------|
| `SET key value`    | Stores a key-value pair in memory    |
| `GET key`          | Retrieves the value of a key         |
| `DEL key` *(optional)* | Deletes a key                      |
| `EXPIRE key seconds` *(optional)* | Sets a time-to-live for a key |

---

## 🔹 Step 5: Choose Technologies & Libraries

| Component         | Choice                                      |
|------------------|--------------------------------------------|
| **Language**     | Rust                                       |
| **TCP Handling** | `std::net::TcpListener` (basic) or `tokio::net::TcpListener` (async) |
| **Concurrency**  | `std::thread` (basic) or `tokio` (async)  |
| **Storage**      | `HashMap<String, String>`                 |
| **Synchronization** | `Arc<Mutex<HashMap<..>>>`             |

---

## 🔹 Step 6: Implementation Plan

### ✅ **Phase 1: Basic TCP Server**
- Open a TCP socket and listen for connections.
- Accept multiple clients.

### ✅ **Phase 2: Implement Key-Value Storage**
- Use a `HashMap` to store key-value pairs.
- Implement `SET` and `GET` commands.

### ✅ **Phase 3: Handle Concurrency**
- **Basic:** Use threads (`std::thread::spawn`).
- **Advanced:** Use async (`tokio::spawn`).

### 🔜 **Phase 4: Add Error Handling & Robust Parsing**
- Handle invalid commands gracefully.
- Ensure proper error messages.

### 🔜 **Phase 5: Add Persistence (Optional)**
- Write key-value pairs to a file.
- Load data at startup.

### 🔜 **Phase 6: Optimize & Extend Features**
- Implement `EXPIRE`, `DEL`, and other Redis-like features.
- Implement **Pub/Sub** or transactions.

