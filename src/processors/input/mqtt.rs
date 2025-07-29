use crate::processors::Processor;

use crate::config::{extract_field_params, extract_param, StageConfig, FieldConfig};
use crate::core::message::Message;
use crate::core::context::ProcessingContext;
use crate::config::ProcessorConfig;

use async_trait::async_trait;
use rumqttc::{AsyncClient, MqttOptions, Event, Packet, QoS};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct MqttInputConfig {
    pub broker_url: String,
    pub topics: Vec<String>,
    pub client_id: Option<String>,
    pub qos: u8,
    pub clean_session: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub field_config: FieldConfig,
}

impl ProcessorConfig for MqttInputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let broker_url = extract_param(&config.parameters, "broker_url", "mqtt://localhost:1883".to_string());
        let topics: Vec<String> = extract_param(&config.parameters, "topics", vec!["#".to_string()]);      
        let client_id = extract_param(&config.parameters, "client_id", None);
        let qos = extract_param(&config.parameters, "qos", 0);
        let clean_session = extract_param(&config.parameters, "clean_session", true);
        let username = extract_param(&config.parameters, "username", None);
        let password = extract_param(&config.parameters, "password", None); 
        let field_config = extract_field_params(&config.parameters);

        Ok(Self {
            broker_url,
            topics,
            client_id,
            qos,
            clean_session,
            username,
            password,
            field_config,
        })
    }

    fn validate(&self) -> anyhow::Result<()> {
        println!("{:?}", self);

        if self.qos > 2 {
            return Err(anyhow::anyhow!("QoS must be between 0 and 2"));
        }

        if self.broker_url.is_empty() {
            return Err(anyhow::anyhow!("Broker URL cannot be empty"));
        }
        if self.topics.is_empty() {
            return Err(anyhow::anyhow!("At least one topic must be specified"));
        }
        
        Ok(())
    }       
}

pub struct MqttInputProcessor {
    name: String,
    config: MqttInputConfig,
    client: Option<AsyncClient>,
    event_loop: Option<Mutex<rumqttc::EventLoop>>,
}

impl MqttInputProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = MqttInputConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            client: None,
            event_loop: None,
        }))
    }

    fn parse_broker_url(&self) -> anyhow::Result<(String, u16)> {
        let url = &self.config.broker_url;
        
        // Handle mqtt:// prefix
        let clean_url = if url.starts_with("mqtt://") {
            &url[7..]
        } else {
            url
        };

        // Split host and port
        if let Some(colon_pos) = clean_url.find(':') {
            let host = clean_url[..colon_pos].to_string();
            let port = clean_url[colon_pos + 1..].parse::<u16>()
                .map_err(|_| anyhow::anyhow!("Invalid port in broker URL: {}", url))?;
            Ok((host, port))
        } else {
            Ok((clean_url.to_string(), 1883)) // Default MQTT port
        }
    }    
}

#[async_trait]
impl Processor for MqttInputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        // Parse broker URL
        let (host, port) = self.parse_broker_url()?;

        // Generate client ID if not provided
        let client_id = self.config.client_id.clone()
            .unwrap_or_else(|| format!("liminal_{}", uuid::Uuid::new_v4()));

        // Create MQTT options
        let mut mqttoptions = MqttOptions::new(&client_id, host, port);
        mqttoptions.set_clean_session(self.config.clean_session);

        // Set credentials if provided
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            mqttoptions.set_credentials(username, password);
        }

        // Create client and event loop
        let (client, eventloop) = AsyncClient::new(mqttoptions, 10);
        
        // Subscribe to topics
        for topic in &self.config.topics {
            let qos = match self.config.qos {
                0 => QoS::AtMostOnce,
                1 => QoS::AtLeastOnce,
                2 => QoS::ExactlyOnce,
                _ => QoS::AtMostOnce,
            };
            
            client.subscribe(topic, qos).await
                .map_err(|e| anyhow::anyhow!("Failed to subscribe to topic '{}': {}", topic, e))?;
            
            tracing::info!("Subscribed to MQTT topic: {} (QoS: {})", topic, self.config.qos);
        }

        self.client = Some(client);
        self.event_loop = Some(Mutex::new(eventloop));

        tracing::info!("MQTT subscriber '{}' initialised", self.name);
        Ok(())  
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {

        if let Some(ref event_loop_mutex) = self.event_loop {
            let mut eventloop = event_loop_mutex.lock().await;

            // Poll the MQTT event loop
            tokio::select! {
                event_result = eventloop.poll() => {
                    match event_result {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            let topic = publish.topic.clone();
                            let payload_bytes = publish.payload.to_vec();
                            
                            // Try to parse payload as JSON, fallback to string
                            let payload = match serde_json::from_slice::<Value>(&payload_bytes) {
                                Ok(json_value) => json_value,
                                Err(_) => {
                                    // If not valid JSON, store as string
                                    match String::from_utf8(payload_bytes) {
                                        Ok(string_value) => Value::String(string_value),
                                        Err(_) => {
                                            // If not valid UTF-8, store as base64
                                            Value::String(base64::encode(&publish.payload))
                                        }
                                    }
                                }
                            };

                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;

                            tracing::info!("Message payload: {:?}", payload);

                            if let Some(output_info) = &context.output {
                                let message = Message {
                                    source: self.name.clone(),
                                    topic: output_info.name.clone(),
                                    payload,
                                    timestamp: now,
                                };

                                let _ = output_info.channel.publish(message).await;
                                tracing::info!("Received MQTT message from topic: {}", topic);
                            }
                        }
                        Ok(_) => {
                            // Todo: Other MQTT events (connect, disconnect, etc.)
                        }
                        Err(e) => {
                            tracing::error!("MQTT connection error: {}", e);
                            // Perhaps we could implement reconnection logic here?
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // No messages received, continue polling
                }
            }
        }

        Ok(())
    }
}