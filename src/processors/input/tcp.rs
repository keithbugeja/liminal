use crate::config::{ProcessorConfig, StageConfig};
use crate::core::context::ProcessingContext;
use crate::core::timing_mixin::{TimingMixin, WithTimingMixin};
use crate::processors::Processor;
use crate::processors::common::tcp::{TcpConfig, TcpConnection};

use async_trait::async_trait;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct TcpInputConfig {
    tcp_config: TcpConfig,
    pub timing: Option<crate::config::TimingConfig>,
}

impl ProcessorConfig for TcpInputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let tcp_config = TcpConfig::from_stage_config(config)?;
        let timing = config.timing.clone();
        Ok(Self { tcp_config, timing })
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.tcp_config.validate()
    }
}

pub struct TcpInputProcessor {
    name: String,
    config: TcpInputConfig,
    timing: TimingMixin,
    connection: TcpConnection,
}

impl TcpInputProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = TcpInputConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        // Create timing mixin from processor configuration
        let timing = TimingMixin::new(processor_config.timing.as_ref());

        let connection = TcpConnection::new(name.to_string(), processor_config.tcp_config.clone());

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            timing,
            connection,
        }))
    }
}

#[async_trait]
impl Processor for TcpInputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!(
            "{}: TCP input processor initialised with timing semantics",
            self.name
        );
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Ensure we have a connection
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

        // Try to receive a message (non-blocking)
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            self.connection.receive_message_with_length_prefix(),
        )
        .await
        {
            Ok(Ok(message_bytes)) => {
                // Single time capture to ensure consistency
                let event_time = SystemTime::now();
                let sequence_id = self.timing.next_sequence_id();

                tracing::debug!(
                    "{}: Received {} byte message",
                    self.name,
                    message_bytes.len()
                );

                // Parse JSON message
                match serde_json::from_slice::<serde_json::Value>(&message_bytes) {
                    Ok(json_value) => {
                        if let Some(output_info) = &context.output {
                            // Create message using timing mixin
                            let message = self
                                .timing
                                .create_message_with_event_time_extraction(
                                    &self.name,
                                    &output_info.name,
                                    json_value,
                                    event_time,
                                )
                                .with_sequence_id(sequence_id);

                            if let Err(e) = output_info.channel.publish(message).await {
                                tracing::warn!("{}: Downstream publish failed: {:?}", self.name, e);
                            } else {
                                tracing::debug!(
                                    "{}: Successfully processed TCP message",
                                    self.name
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("{}: Failed to parse JSON message: {}", self.name, e);
                        tracing::debug!(
                            "{}: Raw message: {:?}",
                            self.name,
                            String::from_utf8_lossy(&message_bytes)
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!("{}: Failed to receive message: {}", self.name, e);

                // Reset connection for reconnection attempt
                self.connection.disconnect();

                if !self.connection.should_reconnect() {
                    return Err(e);
                }
            }
            Err(_) => {
                // Timeout is expected - just means no message available
                tracing::trace!("{}: No message received (timeout)", self.name);
            }
        }

        Ok(())
    }
}

impl WithTimingMixin for TcpInputProcessor {
    fn timing_mixin(&self) -> &TimingMixin {
        &self.timing
    }

    fn timing_mixin_mut(&mut self) -> &mut TimingMixin {
        &mut self.timing
    }
}
