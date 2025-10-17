

// sx_cmd: Used by our async TCP handlers to send new orders or cancels to the engine.
// rx_cmd: Owned by the engine thread; it receives one Command at a time.
let (sx_cmd, rx_cmd) = crossbeam::channel::unbound::<Command>();
