use crate::core::channel::PubSubChannel;
use crate::core::channel::Subscriber;
use crate::core::message::Message;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait Processor: Send + Sync {
    async fn init(&mut self) -> anyhow::Result<()>;
    async fn process(
        &mut self,
        inputs: Vec<Subscriber<Message>>,
        output: Option<Arc<dyn PubSubChannel<Message>>>,
    ) -> anyhow::Result<()>;
}
