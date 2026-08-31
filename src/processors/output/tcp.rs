use crate::config::{ProcessorConfig, StageConfig};
use crate::core::context::ProcessingContext;
use crate::core::input_poll::{DEFAULT_MAX_DRAIN_PER_INPUT, drain_bounded_each, idle_sleep};
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
        if context.inputs.is_empty() {
            idle_sleep().await;
            return Ok(());
        }

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

        let mut messages_sent = 0;

        for input_message in
            drain_bounded_each(&mut context.inputs, DEFAULT_MAX_DRAIN_PER_INPUT).await
        {
            tracing::debug!(
                "{}: Processing message from {}",
                self.name,
                input_message.message.source
            );

            // Convert message to JSON and encode as UTF-8
            let json_value = serde_json::json!({
                "source": input_message.message.source,
                "topic": input_message.message.topic,
                "payload": input_message.message.payload,
                "timestamp": input_message.message.timestamp
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
                messages_sent += 1;
            }
        }

        if messages_sent == 0 {
            idle_sleep().await;
        }

        Ok(())
    }
}
