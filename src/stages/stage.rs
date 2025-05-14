use crate::core::message::Message;
use async_trait::async_trait;

pub enum Input<Message> {
    Broadcast(tokio::sync::broadcast::Receiver<Message>),
    Mpsc(tokio::sync::mpsc::Receiver<Message>),
}

#[async_trait]
pub trait Stage: Send + Sync {
    fn name(&self) -> &str;

    fn attach_input(&mut self, input: Input<Message>);

    async fn init(&mut self) -> anyhow::Result<()>;
    async fn run(self: Box<Self>) -> anyhow::Result<()>;
}
