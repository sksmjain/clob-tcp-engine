use std::collections::{BTreeMap, HashMap, VecDeque};

enum Side {
    Bid,
    Ask
}

struct Order {
    id: u64,
    client_id: u64,
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