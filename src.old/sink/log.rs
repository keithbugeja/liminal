use async_trait::async_trait;
use tracing::info;

use super::OutputSinkHandler;
use crate::core::message::Message;

pub struct LogSink {
    pub name: String,
}

impl LogSink {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl OutputSinkHandler for LogSink {
    async fn handle(&self, message: Message) {
        info!(target: "sink", "[{}]: {:?}", self.name, message);
    }
}
