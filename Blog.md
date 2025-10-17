# Building a Deterministic Matching Engine: Why We Chose TCP (and Rust) Over HTTP
Trading systems live and die by latency, determinsm and trust.
When I set out to build a Centeral Limit Order Book (CLOB) engine, I knew the tech stack couldn't just be convinient - it had to be correct under pressure.

That simple constraint ruled out the usual web dev default.
So instead of spinning up an HTTP API, I'm building a raw TCP server in Rust, designed to handle orders, matches, and market-data fanout at microsencond scale.

This post explains why:
* Why HTTP wasn’t good enough,
* Why raw TCP makes sense for a CLOB, and
* Why Rust is the language we trust to make it safe.

## Why HTTP isn’t fit for the hot path
When most developers think about building an API service, HTTP is the default choice. Its well understood, easy to debug, and universally supported - browsers, SDKs, curl - everthing speaks HTTP.

So it's fair to ask:
Why are we building our Central Limit Order Book (CLOB) on a custom TCP server instead?

The short answer: performance, determinism and control
The long answer reveals how the design of trading system differs from ordinary web apps.

---
### The problem with HTTP for matching engines
At the heart of any exchange or CLOB is one invariant.
The system must process orders in strict arrival order, with minimum latency, and zero ambiguity.

That sounds simple - until you realize that the web's favourite protocol, HTTP, is designed for something entirely different: request/response transactions, not streaming events.