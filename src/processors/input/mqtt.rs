use crate::processors::Processor;
use crate::processors::common::MqttConnectionConfig;
use crate::config::{
    extract_field_params, 
    extract_param, 
    ProcessorConfig,
    StageConfig, 
    FieldConfig
};
use crate::core::message::Message;
use crate::core::context::ProcessingContext;
use crate::core::time::now_millis;

use async_trait::async_trait;
use rumqttc::{AsyncClient, Event, Packet};
use serde_json::Value;
use tokio::time::Duration;
use tokio::sync::Mutex;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

#[derive(Debug, Clone)]
pub struct MqttInputConfig {
    pub connection: MqttConnectionConfig,
    pub topics: Vec<String>,
    pub field_config: FieldConfig,
}

impl ProcessorConfig for MqttInputConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let connection = MqttConnectionConfig::from_parameters(&config.parameters, "liminal");
        let topics: Vec<String> = extract_param(&config.parameters, "topics", vec!["#".to_string()]);      
        
        // Field configuration will be removed. Any payload parameter renaming should be handled by 
        // a separate rename processor. Will be changing this to None in the future.
        let field_config = extract_field_params(&config.parameters);

        Ok(Self {
            connection,
            topics,
            field_config,
        })
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.connection.validate()?;
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
}

#[async_trait]
impl Processor for MqttInputProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        let mqttoptions = self.config.connection.create_mqtt_options("liminal")?;
        let (client, eventloop) = AsyncClient::new(mqttoptions, 10);
                
        for topic in &self.config.topics {
            client
                .subscribe(topic, self.config.connection.qos())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to subscribe to topic '{}': {}", topic, e))?;
            tracing::info!("Subscribed to MQTT topic: {} (QoS: {})", topic, self.config.connection.qos);
        }

        self.client = Some(client);
        self.event_loop = Some(Mutex::new(eventloop));

        tracing::info!("Field configuration: {:?}", self.config.field_config);

        tracing::info!("MQTT subscriber '{}' initialised", self.name);
        Ok(())  
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        if let Some(ref event_loop_mutex) = self.event_loop {
            // Changing logic to poll under the lock but then drop it before
            // any downstram awaits, to avoid convoying stages.
            let (maybe_topic, maybe_payload_bytes) = {
                let mut eventloop = event_loop_mutex.lock().await;
            
                tokio::select! {
                    event_result = eventloop.poll() => {
                        match event_result {
                            Ok(Event::Incoming(Packet::Publish(publish))) => {
                                (Some(publish.topic.clone()), Some(publish.payload.to_vec()))
                            }
                            Ok(_) => (None, None),
                            Err(e) => {
                                tracing::error!("MQTT connection error: {}", e);
                                (None, None)
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => (None, None),
                }
            };

            // Process downstream messages, if any
            if let (Some(topic), Some(payload_bytes)) = (maybe_topic, maybe_payload_bytes) {
                let payload = match serde_json::from_slice::<Value>(&payload_bytes) {
                    Ok(json_value) => json_value,
                    Err(_) => match std::str::from_utf8(&payload_bytes) {
                        Ok(s) => Value::String(s.to_owned()),
                        Err(_) => Value::String(BASE64.encode(&payload_bytes)),
                    }
                };

                let now = now_millis();

                tracing::debug!("MQTT '{}' payload: {},", topic, payload);

                if let Some(output_info) = &context.output {
                    let message = Message {
                        source: self.name.clone(),
                        topic: output_info.name.clone(),
                        payload,
                        timestamp: now,
                    };

                    if let Err(e) = output_info.channel.publish(message).await {
                        tracing::warn!("Downstream publish failed: {:?}", e);
                    } else {
                        tracing::info!("Received MQTT message from topic: {}", topic); // Might downcast to debug later
                    }    
                }
            }
        }

        Ok(())
    }
}