use crate::processors::Processor;
use crate::processors::common::MqttConnectionConfig;
use crate::config::{extract_param, StageConfig};
use crate::config::ProcessorConfig;
use crate::core::context::ProcessingContext;

use async_trait::async_trait;
use rumqttc::{AsyncClient};
use serde_json::Value;
use std::collections::HashMap;
use tokio::select;

#[derive(Debug, Clone)]
pub struct MqttOutputConfig {
    pub connection: MqttConnectionConfig,
    pub topic_map: HashMap<String, String>,
    pub default_topic: Option<String>,
    pub retain: bool,
}

impl ProcessorConfig for MqttOutputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let connection = MqttConnectionConfig::from_parameters(&config.parameters, "liminal_out");
        
        // Extract topic mapping from config
        let topic_map: HashMap<String, String> = extract_param(
            &config.parameters, 
            "topic_map", 
            HashMap::new()
        );
        
        // Optional default topic for unmapped inputs
        let default_topic: Option<String> = extract_param(
            &config.parameters, 
            "default_topic", 
            None
        );
        
        let retain = extract_param(&config.parameters, "retain", false);

        Ok(Self {
            connection,
            topic_map,
            default_topic,
            retain,
        })
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.connection.validate()?;
        
        if self.topic_map.is_empty() && self.default_topic.is_none() {
            return Err(anyhow::anyhow!("Must specify either topic_map or default_topic"));
        }
        
        // Validate all topics are non-empty
        for (input, topic) in &self.topic_map {
            if topic.is_empty() {
                return Err(anyhow::anyhow!("Topic for input '{}' cannot be empty", input));
            }
        }
        
        Ok(())
    }
}

pub struct MqttOutputProcessor {
    name: String,
    config: MqttOutputConfig,
    client: Option<AsyncClient>,
}

impl MqttOutputProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = MqttOutputConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            client: None,
        }))
    }

    fn resolve_topic(&self, channel_name: &str) -> Option<&str> {
        // First try the topic map, then fall back to default
        self.config.topic_map.get(channel_name)
            .map(|s| s.as_str())
            .or_else(|| self.config.default_topic.as_deref())
    }

    fn format_payload(&self, payload: &Value) -> anyhow::Result<String> {
        // Convert payload to JSON string for MQTT transmission
        serde_json::to_string(payload)
            .map_err(|e| anyhow::anyhow!("Failed to serialize payload: {}", e))
    }
}

#[async_trait]
impl Processor for MqttOutputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        let mqttoptions = self.config.connection.create_mqtt_options("liminal_out")?;

        // Create client and event loop
        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
        
        // Spawn the event loop in a background task to handle MQTT connection
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {
                        // Event loop running normally
                    }
                    Err(e) => {
                        tracing::error!("MQTT event loop error: {:?}", e);
                        // Small delay before retrying
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    }
                }
            }
        });
        
        self.client = Some(client);

        tracing::info!(
            "MQTT publisher '{}' initialised (broker: {}, topic_map: {:?}, default: {:?}, QoS: {}, retain: {})",
            self.name, 
            self.config.connection.broker_url, 
            self.config.topic_map,
            self.config.default_topic,
            self.config.connection.qos, 
            self.config.retain
        );
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        if let Some(ref client) = self.client {
            let mut messages_published = 0;

            // Process all input channels
            for (channel_name, input) in context.inputs.iter_mut() {
                select! {
                    message = input.recv() => {
                        if let Some(message) = message {
                            // Resolve topic using channel name
                            if let Some(topic) = self.resolve_topic(channel_name) {
                                // Format payload as JSON string
                                let payload_str = self.format_payload(&message.payload)?;
                                
                                // Publish to MQTT broker
                                if let Err(e) = client.publish(
                                    topic, 
                                    self.config.connection.qos(), 
                                    self.config.retain, 
                                    payload_str.as_bytes()
                                ).await {
                                    tracing::error!("Failed to publish to MQTT topic '{}': {:?}", topic, e);
                                } else {
                                    tracing::debug!(
                                        "Published message from '{}' to MQTT topic: {} (payload: {})", 
                                        channel_name, topic, payload_str
                                    );
                                    messages_published += 1;
                                }
                            } else {
                                tracing::warn!("No topic mapping found for input channel: {}", channel_name);
                            }
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                        // Timeout - no messages received, continue processing
                        break;
                    }
                }
            }

            // Small delay to prevent busy-waiting when no messages
            if messages_published == 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }

        Ok(())
    }
}