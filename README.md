🏗️ Central Limit Order Book (CLOB) TCP Server — Rust Blueprint
1. Core Idea

Rust engine (CLOB matcher) runs as a TCP server with a binary protocol.

Gateway (WebSocket bridge) exposes wss:// for browsers, proxies to engine.

Binary protocol (length-prefixed frames): compact, low-latency, replayable.

WAL + snapshots: append-only log for recovery and determinism.
