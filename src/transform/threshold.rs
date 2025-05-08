use super::Transformer;
use crate::message::Message;
use async_trait::async_trait;

#[derive(Default)]
pub struct ThresholdTransformer;

#[async_trait]
impl Transformer for ThresholdTransformer {
    async fn apply(&self, input: Message) -> Option<Message> {
        if let Some(v) = input.payload.get("counter") {
            if let Some(n) = v.as_u64() {
                return if n >= 5 { Some(input) } else { None };
            }
        }
        Some(input)
    }
}
