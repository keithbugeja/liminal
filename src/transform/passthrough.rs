use super::Transformer;
use crate::message::Message;
use async_trait::async_trait;

pub struct PassthroughTransformer;

#[async_trait]
impl Transformer for PassthroughTransformer {
    async fn apply(&self, input: Message) -> Option<Message> {
        Some(input) // nop
    }
}
