use crate::types::{Command, Event, Order, OrderBook};
use crossbeam::channel::{Receiver, Sender};

// ========================== Minimal Engine (stub) ==========================
// Just responds so your gateway path compiles & runs.
pub fn run_engine(rx_cmd: Receiver<Command>, _tx_bcast: Sender<Event>) {
    while let Ok(cmd) = rx_cmd.recv() {
        match cmd {
            Command::Ping(sink) => { let _ = sink.send(Event::Pong); }
            Command::Order(no, sink) => {
                let _ = sink.send(Event::Ack {ord_id: no.id, note: "ok" });
            }
            Command::Cancel { ord_id, sink, .. } => {
                let _ = sink.send(Event::Reject {ord_id, reason: "not_found" });
            }
        }
    }
}


fn handle_new(no: Order, b: &mut OrderBook, sink: &Sender<Event>, tx_md: &Sender<Event>) {

}

fn handle_cancel(cl_id: u64, b: &mut OrderBook, tx_md: &Sender<Event>) -> bool {
    false
}