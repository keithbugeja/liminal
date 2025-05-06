use crate::config::Transformation;
use crate::message::Message;

use async_trait::async_trait;

pub mod simple;

pub fn create_transform(
    kind: &str,
    params: &Option<std::collections::HashMap<String, serde_json::Value>>,
) -> Result<Box<dyn Transform>, String> {
    match kind {
        "passthrough" => Ok(Box::new(simple::Passthrough)),
        "threshold" => Ok(Box::new(simple::Threshold::default())),
        _ => Err(format!("Unknown transform kind '{}'", kind)),
    }
}

#[async_trait]
pub trait Transform: Send + Sync {
    async fn apply(&self, input: Message) -> Option<Message>;
}
