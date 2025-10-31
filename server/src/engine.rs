use std::collections::VecDeque;
use std::time::Duration;
use std::fmt::Write;
use crossbeam::channel::{Receiver, Sender, tick, select};
use tracing::{info, warn};
use crate::types::{Command, Event, Order, OrderBook, Side, Tif};

/// Engine main loop: single thread, deterministic execution
pub fn run_engine(rx_cmd: Receiver<Command>, tx_md: Sender<Event>) {
    info!("[engine] ✅ Engine started — waiting for incoming commands...");

    let mut book = OrderBook::default();
    info!("[engine] OrderBook summary => bids={}, asks={}", book.bids.len(), book.asks.len());

    // 🔔 5s heartbeat
    let ticker = tick(Duration::from_secs(5));

    loop {
        select! {
            recv(rx_cmd) -> msg => {
                let cmd = match msg {
                    Ok(c) => c,
                    Err(_) => {
                        warn!("[engine] ⚙️ Engine loop terminated (rx closed).");
                        break;
                    }
                };

                match cmd {
                    Command::Ping(sink) => {
                        info!("[engine] 🔁 Received PING");
                        let _ = sink.send(Event::Pong);
                        info!("[engine] 🏓 Sent PONG");
                    }
                    Command::Order(no, sink) => {
                        info!(id=no.id, side=?no.side, price=no.price, qty=no.qty, tif=?no.tif,
                              "[engine] 🆕 New Order");
                        handle_new(no, &mut book, &sink, &tx_md);
                    }
                    Command::Cancel { ord_id, sink, .. } => {
                        info!(ord_id, "[engine] ❌ Cancel Request");
                        if handle_cancel(ord_id, &mut book, &tx_md) {
                            info!(ord_id, "[engine] ✅ Cancel Success");
                            let _ = sink.send(Event::Ack { ord_id, note: "canceled" });
                        } else {
                            warn!(ord_id, "[engine] ⚠️ Cancel Failed — not found");
                            let _ = sink.send(Event::Reject { ord_id, reason: "not_found" });
                        }
                    }
                }
            },
            // ⏱️ every 5 seconds
            recv(ticker) -> _ => {
                info!("{}", summarize_book(&book));
            }
        }
    }
}

// ---- helper: compact book snapshot
fn summarize_book(b: &OrderBook) -> String {
    let mut out = String::new();

    // --- top of book (best levels)
    let best_bid = b.bids
        .iter()
        .next_back()
        .map(|(px, q)| (*px, q.iter().map(|o| o.qty).sum::<u64>()));
    let best_ask = b.asks
        .iter()
        .next()
        .map(|(px, q)| (*px, q.iter().map(|o| o.qty).sum::<u64>()));

    // --- levels and cum quantities
    let bid_levels = b.bids.len();
    let ask_levels = b.asks.len();
    let bid_qty: u64 = b.bids.values().map(|v| v.iter().map(|o| o.qty).sum::<u64>()).sum();
    let ask_qty: u64 = b.asks.values().map(|v| v.iter().map(|o| o.qty).sum::<u64>()).sum();

    // --- pending order counts (number of resting orders)
    let bid_orders: usize = b.bids.values().map(|q| q.len()).sum();
    let ask_orders: usize = b.asks.values().map(|q| q.len()).sum();

    // --- spread
    let spread = match (best_bid, best_ask) {
        (Some((bp, _)), Some((ap, _))) if ap >= bp => Some(ap - bp),
        _ => None,
    };

    let _ = writeln!(
        out,
        "[engine] ⏱️ Book@5s  levels: bids={} asks={}  totals: bid_qty={} ask_qty={}  pending: bid_orders={} ask_orders={}",
        bid_levels, ask_levels, bid_qty, ask_qty, bid_orders, ask_orders
    );

    match best_bid {
        Some((px, lvl)) => { let _ = writeln!(out, "  • best_bid: px={} level_qty={}", px, lvl); }
        None => { let _ = writeln!(out, "  • best_bid: none"); }
    }
    match best_ask {
        Some((px, lvl)) => { let _ = writeln!(out, "  • best_ask: px={} level_qty={}", px, lvl); }
        None => { let _ = writeln!(out, "  • best_ask: none"); }
    }
    if let Some(s) = spread { let _ = writeln!(out, "  • spread: {}", s); }

    out
}

/// Insert a new order:
fn handle_new(mut no: Order, b: &mut OrderBook, sink: &Sender<Event>, tx_md: &Sender<Event>) {
    let mut remaining = no.qty;
    match no.side {
        Side::Bid => {
            info!("[engine] ↕ Matching BID order against ASK levels...");
            while remaining > 0 {
                let Some((&ask_px, _)) = b.asks.iter().next() else {
                    info!("[engine] No ASK levels available — resting remaining order.");
                    break;
                };
                if no.price < ask_px {
                    info!("[engine] Bid price {} < best ask {} — stop crossing.", no.price, ask_px);
                    break;
                }

                let q = b.asks.get_mut(&ask_px).expect("ask level must exist");
                while remaining > 0 {
                    let (maker_ord_id, maker_cl_id, fill, emptied) = {
                        let Some(front) = q.front_mut() else { break; };
                        let fill = remaining.min(front.qty);
                        remaining -= fill;
                        front.qty -= fill;
                        let maker_ord_id = front.id;
                        let maker_cl_id = front.cl_id;
                        let emptied = front.qty == 0;
                        (maker_ord_id, maker_cl_id, fill, emptied)
                    };

                    info!(price=ask_px, qty=fill, taker=no.id, maker=maker_ord_id,
                          "[trade] 💥 TRADE");

                    let trade = Event::Trade {
                        price: ask_px,
                        qty: fill,
                        taker_cl_id: no.cl_id,
                        maker_cl_id,
                    };
                    let _ = sink.send(trade.clone());
                    let _ = tx_md.send(trade);

                    if emptied {
                        q.pop_front();
                        info!("[book] Ask order {} fully filled and removed", maker_ord_id);
                    }
                }

                if q.is_empty() {
                    b.asks.remove(&ask_px);
                    info!("[book] Ask level {} now empty and removed", ask_px);
                }

                let lvl_qty: u64 = b.asks
                    .get(&ask_px)
                    .map(|v| v.iter().map(|o| o.qty).sum::<u64>())
                    .unwrap_or(0u64);
                info!("[book] 📉 Ask Level Update => px={} qty={}", ask_px, lvl_qty);
                let _ = tx_md.send(Event::BookDelta { side: Side::Ask, price: ask_px, level_qty: lvl_qty });
            }

            let ack_id = no.id;
            if remaining > 0 && matches!(no.tif, Tif::Gtc) {
                info!("[book] 📥 Resting BID order => id={} px={} qty={}", no.id, no.price, remaining);
                let rest_px = no.price;
                no.qty = remaining;
                let entry = b.bids.entry(rest_px).or_insert_with(VecDeque::new);
                entry.push_back(no);
                b.lookup.insert(ack_id, (Side::Bid, rest_px));

                let lvl_qty: u64 = entry.iter().map(|o| o.qty).sum();
                info!("[book] 📈 Bid Level Update => px={} qty={}", rest_px, lvl_qty);
                let _ = tx_md.send(Event::BookDelta { side: Side::Bid, price: rest_px, level_qty: lvl_qty });
            }

            info!("[engine] ✅ Ack Bid Order id={}", ack_id);
            let _ = sink.send(Event::Ack { ord_id: ack_id, note: "ok" });
        }

        Side::Ask => {
            info!("[engine] ↕ Matching ASK order against BID levels...");
            while remaining > 0 {
                let Some((&bid_px, _)) = b.bids.iter().next_back() else {
                    info!("[engine] No BID levels available — resting remaining order.");
                    break;
                };
                if no.price > bid_px {
                    info!("[engine] Ask price {} > best bid {} — stop crossing.", no.price, bid_px);
                    break;
                }

                let q = b.bids.get_mut(&bid_px).expect("bid level must exist");
                while remaining > 0 {
                    let (maker_ord_id, maker_cl_id, fill, emptied) = {
                        let Some(front) = q.front_mut() else { break; };
                        let fill = remaining.min(front.qty);
                        remaining -= fill;
                        front.qty -= fill;
                        let maker_ord_id = front.id;
                        let maker_cl_id = front.cl_id;
                        let emptied = front.qty == 0;
                        (maker_ord_id, maker_cl_id, fill, emptied)
                    };

                    info!(price=bid_px, qty=fill, taker=no.id, maker=maker_ord_id,
                          "[trade] 💥 TRADE");

                    let trade = Event::Trade {
                        price: bid_px,
                        qty: fill,
                        taker_cl_id: no.cl_id,
                        maker_cl_id,
                    };
                    let _ = sink.send(trade.clone());
                    let _ = tx_md.send(trade);

                    if emptied {
                        q.pop_front();
                        info!("[book] Bid order {} fully filled and removed", maker_ord_id);
                    }
                }

                if q.is_empty() {
                    b.bids.remove(&bid_px);
                    info!("[book] Bid level {} now empty and removed", bid_px);
                }

                let lvl_qty: u64 = b.bids
                    .get(&bid_px)
                    .map(|v| v.iter().map(|o| o.qty).sum::<u64>())
                    .unwrap_or(0u64);
                info!("[book] 📉 Bid Level Update => px={} qty={}", bid_px, lvl_qty);
                let _ = tx_md.send(Event::BookDelta { side: Side::Bid, price: bid_px, level_qty: lvl_qty });
            }

            let ack_id = no.id;
            if remaining > 0 && matches!(no.tif, Tif::Gtc) {
                info!("[book] 📥 Resting ASK order => id={} px={} qty={}", no.id, no.price, remaining);
                let rest_px = no.price;
                no.qty = remaining;
                let entry = b.asks.entry(rest_px).or_insert_with(VecDeque::new);
                entry.push_back(no);
                b.lookup.insert(ack_id, (Side::Ask, rest_px));

                let lvl_qty: u64 = entry.iter().map(|o| o.qty).sum();
                info!("[book] 📈 Ask Level Update => px={} qty={}", rest_px, lvl_qty);
                let _ = tx_md.send(Event::BookDelta { side: Side::Ask, price: rest_px, level_qty: lvl_qty });
            }

            info!("[engine] ✅ Ack Ask Order id={}", ack_id);
            let _ = sink.send(Event::Ack { ord_id: ack_id, note: "ok" });
        }
    }
}

/// Cancel an existing order by `ord_id`.
fn handle_cancel(ord_id: u64, b: &mut OrderBook, tx_md: &Sender<Event>) -> bool {
    info!("[engine] 🔍 Attempting to cancel order {}", ord_id);
    if let Some((side, px)) = b.lookup.remove(&ord_id) {
        let book_side = match side {
            Side::Bid => &mut b.bids,
            Side::Ask => &mut b.asks,
        };

        if let Some(q) = book_side.get_mut(&px) {
            if let Some(pos) = q.iter().position(|o| o.id == ord_id) {
                q.remove(pos);
                info!("[book] ❎ Order {} removed from {:?} px={}", ord_id, side, px);

                let lvl_qty: u64 = q.iter().map(|o| o.qty).sum();
                info!("[book] 📊 Level Update => side={:?} px={} qty={}", side, px, lvl_qty);
                let _ = tx_md.send(Event::BookDelta { side, price: px, level_qty: lvl_qty });

                if q.is_empty() {
                    book_side.remove(&px);
                    info!("[book] Level {} {:?} now empty — removed", px, side);
                }
                return true;
            }
        }
    }
    warn!("[engine] ⚠️ Cancel failed — order {} not found", ord_id);
    false
}
