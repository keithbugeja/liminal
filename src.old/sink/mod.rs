use crate::config::OutputSink;
use crate::core::message::Message;

use async_trait::async_trait;

pub mod log;

#[async_trait]
pub trait OutputSinkHandler: Send + Sync {
    async fn handle(&self, message: Message);
}

pub fn create_output_sink(
    name: &str,
    config: &OutputSink,
) -> Result<Box<dyn OutputSinkHandler>, String> {
    match config.kind.as_str() {
        "log" => Ok(Box::new(log::LogSink::new(name))),
        _ => Err(format!("Unknown sink kind '{}'", config.kind)),
    }
}
