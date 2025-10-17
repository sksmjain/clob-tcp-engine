# CLOB TCP Engine

A high-performance Central Limit Order Book (CLOB) engine implemented in Rust with a binary TCP protocol for ultra-low latency order matching.

## 🏗️ Architecture

- **Rust Engine**: Core CLOB matcher running as a TCP server with binary protocol
- **Binary Protocol**: Length-prefixed frames for compact, low-latency communication
- **Async Processing**: Multi-threaded Tokio runtime for concurrent client handling
- **Future Gateway**: WebSocket bridge planned to expose `wss://` for browser clients

```
──────────────────────────────────────────────────────────────────────────────
(1) Client sends TCP frame with NEW_ORDER
──────────────────────────────────────────────────────────────────────────────
        |
        ▼
 ┌──────────────────────────────────────────┐
 │ Tokio task parses bytes                  │
 │ builds Command::NewOrder(order, sink)    │
 │                                          │
 │     tx_cmd.send(Command)  ─────────────► │   (crossbeam channel)
 └──────────────────────────────────────────┘
        |
        |        (thread boundary)
        ▼
──────────────────────────────────────────────────────────────────────────────
(2) Engine thread receives command
──────────────────────────────────────────────────────────────────────────────
 ┌──────────────────────────────────────────┐
 │ fn run_engine(rx_cmd, tx_broadcast) {    │
 │   while let Ok(cmd) = rx_cmd.recv() {    │
 │     match cmd {                          │
 │       NewOrder(no, sink) => {            │
 │         ... match logic ...              │
 │         sink.send(Event::Ack{cl_ord_id});│
 │         sink.send(Event::Trade{...});    │
 │         tx_broadcast.send(Event::Trade{...});│
 │       }                                  │
 │     }                                    │
 │   }                                      │
 │ }                                        │
 └──────────────────────────────────────────┘
        |
        |   two outputs
        |   ├─ sink (per-client)
        |   └─ tx_broadcast (for market data)
        ▼
──────────────────────────────────────────────────────────────────────────────
(3) Back to gateway → send events to client
──────────────────────────────────────────────────────────────────────────────
 ┌──────────────────────────────────────────┐
 │ Tokio task polls rx_evt_conn.try_recv()  │
 │ for this connection                      │
 │                                          │
 │   while let Ok(evt) = rx_evt_conn.try_recv() { │
 │       send_event_frame(&mut socket, &evt).await?; │
 │   }                                      │
 └──────────────────────────────────────────┘
        |
        ▼
──────────────────────────────────────────────────────────────────────────────
(4) Client receives binary events
──────────────────────────────────────────────────────────────────────────────
   ACK → TRADE → BOOK_DELTA frames
──────────────────────────────────────────────────────────────────────────────
```

## 📁 Project Structure

```
clob-tcp-engine/
├── server/          # Rust TCP server implementation
│   ├── src/
│   │   └── main.rs  # Main server logic with protocol handling
│   └── Cargo.toml   # Rust dependencies
├── client/          # JavaScript test client
│   └── main.js      # Node.js client for testing
└── README.md
```

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+ (for server)
- Node.js 16+ (for client testing)

### Running the Server

```bash
cd server
cargo run
```

The server will start listening on `0.0.0.0:9000`.

### Testing with the Client

```bash
cd client
node main.js
```

## 📡 Binary Protocol

The engine uses a length-prefixed binary protocol for optimal performance:

### Message Format
```
[u32 length][u16 message_type][u16 body_length][body...]
```

### Message Types
- `MSG_PING (1)`: Ping message
- `MSG_ACK (100)`: Acknowledgment response

### Example Flow
1. Client sends ping: `[4][0][1][0][0][0]` (length=4, type=1, no body)
2. Server responds: `[8][0][100][0][4][0][112][111][110][103]` (ACK with "pong")

## 🔧 Development

### Dependencies
- **tokio**: Async runtime with networking
- **anyhow**: Error handling
- **bytes**: Efficient byte buffer manipulation

### Key Features
- **Concurrent Processing**: Each client connection handled in separate task
- **Buffer Management**: Efficient binary frame parsing with `BytesMut`
- **Protocol Parsing**: Length-prefixed message handling with proper bounds checking

### Future Enhancements
- **WAL (Write-Ahead Log)**: Append-only log for recovery and determinism
- **Snapshots**: Periodic state snapshots for fast recovery
- **WebSocket Gateway**: Bridge for browser clients
- **Order Book Logic**: Full CLOB matching engine implementation

## 🎯 Design Goals

- **Ultra-low Latency**: Binary protocol and async Rust for minimal overhead
- **High Throughput**: Multi-threaded processing for concurrent clients
- **Reliability**: Planned WAL and snapshot system for fault tolerance
- **Scalability**: Designed for high-frequency trading workloads

## 📝 License

This project is a blueprint for building high-performance trading engines.
