use crate::core::channel::PubSubChannel;
use crate::core::channel::Subscriber;
use crate::core::message::Message;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait Stage: Send + Sync {
    fn name(&self) -> &str;

    async fn init(&mut self) -> anyhow::Result<()>;
    async fn run(&mut self) -> anyhow::Result<()>;
    async fn stop(&mut self) -> anyhow::Result<()>;

    fn add_input(&mut self, input: Subscriber<Message>);
    fn add_output(&mut self, output: Arc<dyn PubSubChannel<Message>>);
}
