# CLOB TCP Engine

A high-performance Central Limit Order Book (CLOB) engine implemented in Rust with a binary TCP protocol for ultra-low latency order matching.

## Components

#### Client
In our CLOB system, the Client is not just a “user terminal.”
It’s a network participant — the entry point for traders, bots, or external systems to communicate directly with the matching engine.

It speaks to our binary TCP protocol directly, giving it ultra-low latency and deterministic message delivery — unlike REST or WebSocket APIs, which are layered over HTTP.

Each client can:
* Connect to the server via IP/port (using Node.js net or any TCP library).
* Send framed messages such as PING, NEW_ORDER, or CANCEL.
* Receive events like ACK, REJECT, TRADE, and BOOK_DELTA from the engine.

#### Engine
Why spawn a thread at all?

Because your CLOB engine should be deterministic and fast, not async.
Keeping it in its own thread:
* avoids the overhead of Tokio tasks,guarantees single-threaded state mutation (no locks), and makes latency predictable.
* Async I/O (networking) and sync logic (matching) stay cleanly separated.

#### Order Book
our engine runs as a single-threaded loop (run_engine), and inside it we maintain an in-memory data structure called the OrderBook. This OrderBook is where all live (resting) limit orders are stored until they’re either matched, canceled, or expired.

The structure of an order book for a Central Limit Order Book (CLOB) system in Rust typically consists of two primary sides—bids (buy orders) and asks (sell orders)—each organized to allow rapid matching and efficient state querying.
* Order: Each order generally has an identifier, side (bid/ask), price, quantity, and timestamp for price-time priority matching
* Price Levels: Bids are sorted by descending price; asks by ascending price. Within each price level, orders are sorted by time (FIFO) for fair matching.

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

## Tasks & Threads

#### What is Tokio Tasks?
- Is not a real OS thread.
- Its a lightweight async job that runs on top of few worker threads managed by the tokio runtime.
- Tokio tasks handles thousands of sockets and network I/O (async, concurent, efficient)

#### What is Threads?
- A thread is managed by our OS (Linux, Windows, macOS)
- Each thread gets its own stack, registers, and scheduling by the kernal.
- So our CLOB Engine runs its own dedicated thread because it 
    - needs predicatable CPU time
    - should not yield to other async tasks
    - and wants deterministic single threaded order processing
- Our engine thread handles matching logic in strict sequence (sync, deterministic, single core)

#### Why our design uses both?

If everything was async (Tokio-only)
- Matching engine would have to use async locks or channels.
- We’d risk nondeterministic timing (bad for exchanges).
- Harder to benchmark and reason about “what order executed first.”

If everything was threads (no Tokio)
- We’d have to spawn a thread per connection (expensive).
- 1000 clients = 1000 threads = lots of OS overhead.
- Blocking I/O would kill scalability.

By combining both:
- We get async scalability for networking, and deterministic precision for your core logic.

| Part            | Runs where                                     | Why                                                                              |
| --------------- | ---------------------------------------------- | -------------------------------------------------------------------------------- |
| **TCP Gateway** | **Tokio runtime**                              | Each client connection is an async `tokio::spawn` task handling I/O efficiently. |
| **CLOB Engine** | **Dedicated OS thread** (`std::thread::spawn`) | Pure sync logic; deterministic matching; no async waits.                         |

```
               ┌──────────────────────────────────────────────────┐
               │                 Tokio Runtime                    │
               │    (few OS threads, many lightweight tasks)      │
               │                                                  │
client A ─────►│ tokio::spawn(task for conn A)                    │
client B ─────►│ tokio::spawn(task for conn B)                    │
client C ─────►│ tokio::spawn(task for conn C)                    │
               │             │                                   │
               └─────────────┼───────────────────────────────────┘
                             │ tx_cmd.send(Command)
                             ▼
                 ┌──────────────────────────────┐
                 │  Engine thread (std::thread) │
                 │  - rx_cmd.recv()             │
                 │  - match & update book       │
                 │  - sink.send(Event)          │
                 └──────────────────────────────┘
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

Interactive CLI client (recommended):

```bash
cd client
node main.js
```

Then type commands:

```
ping
new client=2 id=2001 side=sell price=101000 qty=5000 tif=gtc
new client=3 id=3001 side=buy  price=101000 qty=2000 tif=ioc
cancel client=2 id=2001
help
```

Load-test client (multi-connection ping/ack):

```bash
node test.js --clients=50 --interval=1000 --duration=30
```

## 📡 Binary Protocol

The engine uses a length-prefixed binary protocol for optimal performance:

### Message Format
```
[u32 length][u16 message_type][u16 body_length][body...]

Where:
- length = bytes of the payload (from message_type to end of body)
- body_length = bytes in body following the 4-byte header (type + body_length)
```

### Message Types
- `1  (PING)`: Ping message (no body)
- `10 (NEW_ORDER)`: Body = `[u64 client_id][u64 cl_ord_id][u8 side][i64 price][i64 qty][u8 tif]`
- `11 (CANCEL)`: Body = `[u64 client_id][u64 cl_ord_id]`

Events (engine → client):
- `100 (ACK)`: Body = `[u64 cl_ord_id][u16 text_len][text...]` (may carry "pong")
- `101 (TRADE)`: Body = `[i64 price][i64 qty][u64 taker_cl_id][u64 maker_cl_id]`
- `102 (BOOK_DELTA)`: Body = `[u8 side][i64 price][i64 level_qty]`
- `199 (REJECT)`: Body = `[u64 cl_ord_id][u16 reason_len][reason...]`

### Example Flow
1. Client sends `PING`
2. Server responds `ACK` with text "pong"

## 🔧 Development

### Dependencies
- **tokio**: Async runtime with networking
- **anyhow**: Error handling
- **bytes**: Efficient byte buffer manipulation
- **hdrhistogram**: Latency histogram (p50/p95/p99) reporter

### Key Features
- **Concurrent Processing**: Each client connection handled in separate task
- **Buffer Management**: Efficient binary frame parsing with `BytesMut`
- **Protocol Parsing**: Length-prefixed message handling with proper bounds checking
- **Latency Metrics**: Background task reports p50/p95/p99 every few seconds

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
