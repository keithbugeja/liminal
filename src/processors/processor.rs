use crate::core::channel::PubSubChannel;
use crate::core::channel::Subscriber;
use crate::core::message::Message;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// This trait defines the interace for a processor in a pipeline stage. It is responsible
/// for processing messages from input channels and sending them to output channels.
#[async_trait]
pub trait Processor: Send + Sync {
    /// Initializes the processor.
    /// This method is guaranteed to be called before any processing occurs.
    async fn init(&mut self) -> anyhow::Result<()>;

    /// Processes messages from the input channels and sends them to the output channel.
    /// # Arguments
    /// * `inputs` - A vector of input channels from which messages are received.
    /// * `output` - An optional output channel to which processed messages are sent.
    /// # Returns
    /// A result indicating success or failure of the processing.
    /// # Note
    /// The `inputs` vector may contain multiple input channels, and the `output` may be `None`.
    /// The 'inputs' vector may be empty if the processor is not connected to any input channels.
    async fn process(
        &mut self,
        inputs: &mut HashMap<String, Subscriber<Message>>,
        output: Option<&Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()>;
}
