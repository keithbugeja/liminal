use super::Transform;
use crate::message::Message;
use async_trait::async_trait;

pub struct Passthrough;

#[async_trait]
impl Transform for Passthrough {
    async fn apply(&self, input: Message) -> Option<Message> {
        Some(input) // nop
    }
}

#[derive(Default)]
pub struct Threshold;

#[async_trait]
impl Transform for Threshold {
    async fn apply(&self, input: Message) -> Option<Message> {
        if let Some(v) = input.payload.get("counter") {
            if let Some(n) = v.as_u64() {
                return if n >= 5 { Some(input) } else { None };
            }
        }
        Some(input)
    }
}
