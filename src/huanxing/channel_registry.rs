//! Global channel registry for HuanXing tenant heartbeat delivery.
//!
//! Provides a process-wide registry of active channels so that
//! tenant heartbeat and notification subsystems can route messages
//! without holding direct references to channel instances.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::channels::traits::Channel;

fn live_channels_registry() -> &'static Mutex<HashMap<String, Arc<dyn Channel>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<dyn Channel>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register all active channels for tenant heartbeat routing.
pub fn register_live_channels(channels_by_name: &HashMap<String, Arc<dyn Channel>>) {
    let mut guard = live_channels_registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.clear();
    for (name, channel) in channels_by_name {
        guard.insert(name.to_ascii_lowercase(), Arc::clone(channel));
    }
}

/// Look up a live channel by name (case-insensitive).
pub fn get_live_channel(name: &str) -> Option<Arc<dyn Channel>> {
    live_channels_registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&name.to_ascii_lowercase())
        .cloned()
}

fn inbound_message_queue()
-> &'static Mutex<Option<tokio::sync::mpsc::Sender<crate::channels::traits::ChannelMessage>>> {
    static QUEUE: OnceLock<
        Mutex<Option<tokio::sync::mpsc::Sender<crate::channels::traits::ChannelMessage>>>,
    > = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(None))
}

/// Register the global inbound message queue for Heartbeat tasks to dispatch synthetic messages.
pub fn register_inbound_queue(
    tx: tokio::sync::mpsc::Sender<crate::channels::traits::ChannelMessage>,
) {
    let mut guard = inbound_message_queue()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *guard = Some(tx);
}

/// Get the global inbound message queue to dispatch synthetic messages.
pub fn get_inbound_queue()
-> Option<tokio::sync::mpsc::Sender<crate::channels::traits::ChannelMessage>> {
    inbound_message_queue()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
