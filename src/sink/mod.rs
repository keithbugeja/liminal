use crate::config::OutputSink;
use crate::message::Message;

use async_trait::async_trait;

pub mod log;

#[async_trait]
pub trait OutputSinkHandler: Send + Sync {
    async fn handle(&self, message: Message);
}

pub fn create_output_sink_handler(
    name: &str,
    def: &OutputSink,
) -> Result<Box<dyn OutputSinkHandler>, String> {
    match def.kind.as_str() {
        "log" => Ok(Box::new(log::LogSink::new(name))),
        _ => Err(format!("Unknown sink kind '{}'", def.kind)),
    }
}
