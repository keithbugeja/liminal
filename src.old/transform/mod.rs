use crate::config::Transform;
use crate::core::message::Message;

use async_trait::async_trait;

pub mod passthrough;
pub mod threshold;

#[async_trait]
pub trait Transformer: Send + Sync {
    async fn apply(&self, input: Message) -> Option<Message>;
}

pub fn create_transform(name: &str, config: &Transform) -> Result<Box<dyn Transformer>, String> {
    match config.kind.as_str() {
        "passthrough" => Ok(Box::new(passthrough::PassthroughTransformer)),
        "threshold" => Ok(Box::new(threshold::ThresholdTransformer::default())),
        _ => Err(format!("Unknown transform kind '{}'", config.kind)),
    }
}
