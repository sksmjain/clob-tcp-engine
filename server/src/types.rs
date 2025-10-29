use std::collections::{BTreeMap, HashMap, VecDeque};

enum Side {
    Bid,
    Ask
}

struct Order {
    id: u64,
    cl_id: u64,
    side: Side,
    price: u64,
    qty: u64,
    timestamp: u64
}

struct OrderBook {
    bids: BTreeMap<u64, VecDeque(Order)>>, // Descending for bids
    asks: BTreeMap<u64, VecDeque<Order>>, // Ascending for asks
    lookup: HashMap<u64, (Side, u64)>, // Fast lookup by IDs: (Side, price)
}

impl Default for OrderBook {fn default() -> Self {Self{bids:BTreeMap::new(), asks:BTreeMap::new(), lookup:HashMap::new()}}}

// Action from engine → gateway → client
// send the same event to the requesting client and
// also broadcast it to market-data subscribers (another channel).
#[derive(Debug, Clone)]
enum Event {
    Ack {ord_id: u64}, // I got your command
    Reject {ord_id: u64, reason: &'static str}, // Couldn't do it
    Trade {price: u64, qty: u64, taker_cl_id: u64, maker_cl_id: u64}, // A fill happened
    BookDelta {side: Side, price: u64, level_qty: u64}, // This price level changed
    Pong, // Just a pong
}

// Action from gateway → engine
enum Command {
    // Place a new order and tell results back through this Sender<Event>
    Order(Order, crossbeam::channel::Sender<Event>),
    // Cancel a specific client order; send result via 'sink'
    Cancel {cl_id: u64, ord_id: u64, sink: crossbeam::channel:Sender<Event>},
    // Just a ping
    Ping(Sender<Event>),
}

/*
Why include the Sender<Event> inside the command?
Because your engine runs in a separate thread and handles many clients. 
Passing the sink makes the engine connection-aware without global maps or locks. 
It can emit client-specific responses without guessing where to send them.
*/