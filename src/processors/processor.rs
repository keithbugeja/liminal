use crate::core::context::ProcessingContext;

use async_trait::async_trait;

/// This trait defines the interace for a processor in a pipeline stage. It is responsible
/// for processing messages from input channels and sending them to output channels.
#[async_trait]
pub trait Processor: Send + Sync {
    /// Initializes the processor.
    /// This method is guaranteed to be called before any processing occurs.
    async fn init(&mut self) -> anyhow::Result<()>;

    /// Processes messages from the input channels and sends them to the output channel.
    /// 
    /// # Arguments
    /// * `context` - A mutable reference to the processing context, which contains information
    ///   about the input and output channels.
    /// 
    /// # Returns
    /// A result indicating success or failure of the processing.
    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()>;
}
