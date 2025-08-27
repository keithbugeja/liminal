use crate::config::{ProcessorConfig, StageConfig};
use crate::core::context::ProcessingContext;
use crate::processors::Processor;
use crate::processors::common::tcp::{TcpConfig, TcpConnection};

use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct TcpOutputConfig {
    tcp_config: TcpConfig,
}

impl ProcessorConfig for TcpOutputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let tcp_config = TcpConfig::from_stage_config(config)?;
        Ok(Self { tcp_config })
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.tcp_config.validate()
    }
}

pub struct TcpOutputProcessor {
    name: String,
    connection: TcpConnection,
}

impl TcpOutputProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = TcpOutputConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        let connection = TcpConnection::new(name.to_string(), processor_config.tcp_config);

        Ok(Box::new(Self {
            name: name.to_string(),
            connection,
        }))
    }
}

#[async_trait]
impl Processor for TcpOutputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("{}: TCP output processor initialised", self.name);
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Check if we have any messages to process first
        let mut has_messages = false;
        for (_, _subscriber) in &context.inputs {
            // Quick check without consuming messages
            if !context.inputs.is_empty() {
                has_messages = true;
                break;
            }
        }

        // Only try to connect if we have messages to send or we're already connected
        if has_messages || self.connection.is_connected() {
            if let Err(e) = self.connection.ensure_connection().await {
                if self.connection.should_reconnect() {
                    tracing::debug!(
                        "{}: Connection failed, will retry in {}ms: {}",
                        self.name,
                        self.connection.reconnect_interval(),
                        e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        self.connection.reconnect_interval(),
                    ))
                    .await;
                    return Ok(());
                } else {
                    return Err(e);
                }
            }
        }

        // Process messages from inputs
        for (_input_name, subscriber) in &mut context.inputs {
            while let Some(message) = subscriber.try_recv().await {
                tracing::debug!("{}: Processing message from {}", self.name, message.source);

                // Convert message to JSON and encode as UTF-8
                let json_value = serde_json::json!({
                    "source": message.source,
                    "topic": message.topic,
                    "payload": message.payload,
                    "timestamp": message.timestamp
                });
                let json_string = serde_json::to_string(&json_value)?;
                let json_bytes = json_string.into_bytes(); // UTF-8 encoding

                tracing::debug!("{}: Sending {} byte message", self.name, json_bytes.len());

                if let Err(e) = self
                    .connection
                    .send_message_with_length_prefix(&json_bytes)
                    .await
                {
                    tracing::error!("{}: Failed to send message: {}", self.name, e);

                    // Reset connection for reconnection attempt
                    self.connection.disconnect();

                    if !self.connection.should_reconnect() {
                        return Err(e);
                    }
                    break; // Exit message processing loop to attempt reconnection
                } else {
                    tracing::debug!("{}: Successfully sent message", self.name);
                }
            }
        }

        Ok(())
    }
}
