use std::collections::{BTreeMap, HashMap, VecDeque};
use crossbeam::channel::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Tif {
    Gtc,
    Ioc,
}

pub struct Order {
    pub id: u64,
    pub cl_id: u64,
    pub side: Side,
    pub price: u64,
    pub qty: u64,
    #[allow(dead_code)]
    pub timestamp: u64,
    pub tif: Tif,
}

pub struct OrderBook {
    pub bids: BTreeMap<u64, VecDeque<Order>>, // Descending for bids
    pub asks: BTreeMap<u64, VecDeque<Order>>, // Ascending for asks
    pub lookup: HashMap<u64, (Side, u64)>, // Fast lookup by IDs: (Side, price)
}

impl Default for OrderBook {fn default() -> Self {Self{bids:BTreeMap::new(), asks:BTreeMap::new(), lookup:HashMap::new()}}}

// Action from engine → gateway → client
// send the same event to the requesting client and
// also broadcast it to market-data subscribers (another channel).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Event {
    Ack {ord_id: u64, note: &'static str }, // I got your command
    Reject {ord_id: u64, reason: &'static str}, // Couldn't do it
    Trade {price: u64, qty: u64, taker_cl_id: u64, maker_cl_id: u64}, // A fill happened
    BookDelta {side: Side, price: u64, level_qty: u64}, // This price level changed
    Pong, // Just a pong
}

// Action from gateway → engine
#[allow(dead_code)]
pub enum Command {
    // Place a new order and tell results back through this Sender<Event>
    Order(Order, crossbeam::channel::Sender<Event>),
    // Cancel a specific client order; send result via 'sink'
    Cancel {cl_id: u64, ord_id: u64, sink: crossbeam::channel::Sender<Event>},
    // Just a ping
    Ping(Sender<Event>),
}

/*
Why include the Sender<Event> inside the command?
Because your engine runs in a separate thread and handles many clients. 
Passing the sink makes the engine connection-aware without global maps or locks. 
It can emit client-specific responses without guessing where to send them.
*/