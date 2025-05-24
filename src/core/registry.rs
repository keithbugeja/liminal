use crate::config::types::ChannelType;
use crate::core::channel::Channel;

use std::collections::HashMap;
use std::sync::Arc;

pub struct ChannelRegistry<M> {
    channels: HashMap<String, Arc<Channel<M>>>,
}

impl<M> ChannelRegistry<M>
where
    M: Clone + Send + Sync + 'static,
{
    /// Create a new, empty ChannelRegistry.
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Get or create a channel by name.
    ///
    /// If the channel already exists, it returns an `Arc` reference to the existing channel.
    /// Otherwise, it creates a new channel with the specified type and capacity.
    pub fn get_or_create(
        &mut self,
        name: &str,
        channel_type: ChannelType,
        capacity: usize,
    ) -> Arc<Channel<M>> {
        self.channels
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Channel::new(channel_type, capacity)))
            .clone()
    }

    /// Get an existing channel by name.
    ///
    /// Returns `None` if the channel does not exist.
    pub fn get(&self, name: &str) -> Option<Arc<Channel<M>>> {
        self.channels.get(name).cloned()
    }
}
