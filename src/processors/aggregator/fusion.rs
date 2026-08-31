use crate::config::{ProcessorConfig, StageConfig, defaulted_param};
use crate::core::context::ProcessingContext;
use crate::core::message::Message;
use crate::processors::Processor;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use tokio::time::{Duration, sleep};

const MAX_MESSAGES_DRAINED_PER_INPUT: usize = 512;

#[derive(Debug, Clone, Deserialize)]
struct FusionConfig {
    mode: FusionMode,
    conflict_strategy: FusionConflictStrategy,
    join_window_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FusionMode {
    MergeObjects,
    NestByInput,
    LatestByInput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FusionConflictStrategy {
    Prefix,
    Overwrite,
}

impl ProcessorConfig for FusionConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        let fusion_config = Self {
            mode: defaulted_param(&config.parameters, "mode", FusionMode::MergeObjects)?,
            conflict_strategy: defaulted_param(
                &config.parameters,
                "conflict_strategy",
                FusionConflictStrategy::Prefix,
            )?,
            join_window_ms: defaulted_param(&config.parameters, "join_window_ms", 25)?,
        };

        fusion_config.validate()?;
        Ok(fusion_config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.join_window_ms > 60_000 {
            return Err(anyhow::anyhow!(
                "join_window_ms must be less than or equal to 60000"
            ));
        }

        Ok(())
    }
}

pub struct FusionStage {
    name: String,
    config: FusionConfig,
    latest_messages: HashMap<String, Message>,
}

impl FusionStage {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = FusionConfig::from_stage_config(&config)?;

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            latest_messages: HashMap::new(),
        }))
    }
}

#[async_trait]
impl Processor for FusionStage {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!(
            "Fusion processor '{}' initialised with mode {:?}",
            self.name,
            self.config.mode
        );
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        if context.inputs.is_empty() {
            sleep(Duration::from_millis(10)).await;
            return Ok(());
        }

        if matches!(self.config.mode, FusionMode::LatestByInput) {
            return self.process_latest_by_input(context).await;
        }

        let mut messages = drain_latest_ready_inputs(context).await;
        if messages.is_empty() {
            sleep(Duration::from_millis(10)).await;
            return Ok(());
        }

        if self.config.join_window_ms > 0 {
            sleep(Duration::from_millis(self.config.join_window_ms)).await;
            messages.extend(drain_latest_ready_inputs(context).await);
        }

        let Some(output_info) = &context.output else {
            return Ok(());
        };

        let mut fused_message = messages[0].1.clone().mark_processed_by(&self.name);
        fused_message.topic = output_info.name.clone();
        fused_message.payload = fuse_payloads(messages, &self.config);

        output_info
            .channel
            .publish(fused_message)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to publish fused message: {:?}", error))?;

        Ok(())
    }
}

impl FusionStage {
    async fn process_latest_by_input(
        &mut self,
        context: &mut ProcessingContext,
    ) -> anyhow::Result<()> {
        let messages = drain_latest_ready_inputs(context).await;
        if messages.is_empty() {
            sleep(Duration::from_millis(10)).await;
            return Ok(());
        }

        let mut fused_message = messages[0].1.clone().mark_processed_by(&self.name);
        for (input_name, message) in messages {
            self.latest_messages.insert(input_name, message);
        }

        if self.latest_messages.len() < context.inputs.len() {
            return Ok(());
        }

        let Some(output_info) = &context.output else {
            return Ok(());
        };

        let latest_messages = context
            .inputs
            .keys()
            .filter_map(|input_name| {
                self.latest_messages
                    .get(input_name)
                    .cloned()
                    .map(|message| (input_name.clone(), message))
            })
            .collect::<Vec<_>>();

        fused_message.topic = output_info.name.clone();
        fused_message.payload = fuse_payloads(latest_messages, &self.config);

        output_info
            .channel
            .publish(fused_message)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to publish fused message: {:?}", error))?;

        Ok(())
    }
}

async fn drain_latest_ready_inputs(context: &mut ProcessingContext) -> Vec<(String, Message)> {
    let mut messages = Vec::new();

    for (input_name, input) in context.inputs.iter_mut() {
        let mut latest_message = None;
        for _ in 0..MAX_MESSAGES_DRAINED_PER_INPUT {
            let Some(message) = input.try_recv().await else {
                break;
            };
            latest_message = Some(message);
        }

        if let Some(message) = latest_message {
            messages.push((input_name.clone(), message));
        }
    }

    messages
}

fn fuse_payloads(messages: Vec<(String, Message)>, config: &FusionConfig) -> Value {
    match config.mode {
        FusionMode::MergeObjects => Value::Object(merge_object_payloads(messages, config)),
        FusionMode::NestByInput | FusionMode::LatestByInput => {
            let mut fused = Map::new();
            for (input_name, message) in messages {
                fused.insert(input_name, message.payload);
            }
            Value::Object(fused)
        }
    }
}

fn merge_object_payloads(
    messages: Vec<(String, Message)>,
    config: &FusionConfig,
) -> Map<String, Value> {
    let mut fused = Map::new();

    for (input_name, message) in messages {
        match message.payload {
            Value::Object(fields) => {
                for (field_name, value) in fields {
                    insert_fused_field(&mut fused, &input_name, field_name, value, config);
                }
            }
            value => {
                insert_fused_field(&mut fused, &input_name, input_name.clone(), value, config);
            }
        }
    }

    fused
}

fn insert_fused_field(
    fused: &mut Map<String, Value>,
    input_name: &str,
    field_name: String,
    value: Value,
    config: &FusionConfig,
) {
    if !fused.contains_key(&field_name)
        || matches!(config.conflict_strategy, FusionConflictStrategy::Overwrite)
    {
        fused.insert(field_name, value);
        return;
    }

    fused.insert(format!("{}_{}", input_name, field_name), value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ChannelType;
    use crate::config::types::StageConfig;
    use crate::core::channel::{Channel, PubSubChannel, Subscriber};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn fusion_merges_ready_object_payloads() {
        let mut processor = FusionStage::new("fusion", fusion_stage_config(None)).unwrap();
        let mut context = ProcessingContext::new("fusion".to_string());
        let (first_sender, first_receiver) = mpsc::channel(4);
        let (second_sender, second_receiver) = mpsc::channel(4);
        let output = Arc::new(Channel::new(ChannelType::Broadcast, 4));
        let mut output_receiver = output.subscribe();

        context.add_input("temperature".to_string(), Subscriber::Mpsc(first_receiver));
        context.add_input("humidity".to_string(), Subscriber::Mpsc(second_receiver));
        context.attach_output("fusion_data".to_string(), output);

        first_sender
            .send(Message::new(
                "temp",
                "temperature",
                json!({ "temperature": 22.5 }),
            ))
            .await
            .unwrap();
        second_sender
            .send(Message::new(
                "humidity",
                "humidity",
                json!({ "humidity": 54 }),
            ))
            .await
            .unwrap();

        processor.process(&mut context).await.unwrap();

        let message = output_receiver.recv().await.unwrap();
        assert_eq!(message.source, "fusion");
        assert_eq!(message.topic, "fusion_data");
        assert_eq!(message.payload["temperature"], json!(22.5));
        assert_eq!(message.payload["humidity"], json!(54));
    }

    #[tokio::test]
    async fn fusion_prefixes_colliding_fields_by_default() {
        let mut processor = FusionStage::new("fusion", fusion_stage_config(None)).unwrap();
        let mut context = ProcessingContext::new("fusion".to_string());
        let (first_sender, first_receiver) = mpsc::channel(4);
        let (second_sender, second_receiver) = mpsc::channel(4);
        let output = Arc::new(Channel::new(ChannelType::Broadcast, 4));
        let mut output_receiver = output.subscribe();

        context.add_input("left".to_string(), Subscriber::Mpsc(first_receiver));
        context.add_input("right".to_string(), Subscriber::Mpsc(second_receiver));
        context.attach_output("fusion_data".to_string(), output);

        first_sender
            .send(Message::new("left", "left", json!({ "value": 1 })))
            .await
            .unwrap();
        second_sender
            .send(Message::new("right", "right", json!({ "value": 2 })))
            .await
            .unwrap();

        processor.process(&mut context).await.unwrap();

        let message = output_receiver.recv().await.unwrap();
        assert!(message.payload.get("value").is_some());
        assert!(
            message.payload.get("left_value").is_some()
                || message.payload.get("right_value").is_some()
        );
    }

    #[tokio::test]
    async fn fusion_can_nest_payloads_by_input() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_string(), json!("nest_by_input"));
        parameters.insert("join_window_ms".to_string(), json!(0));
        let mut processor =
            FusionStage::new("fusion", fusion_stage_config(Some(parameters))).unwrap();
        let mut context = ProcessingContext::new("fusion".to_string());
        let (sender, receiver) = mpsc::channel(4);
        let output = Arc::new(Channel::new(ChannelType::Broadcast, 4));
        let mut output_receiver = output.subscribe();

        context.add_input("sensor".to_string(), Subscriber::Mpsc(receiver));
        context.attach_output("fusion_data".to_string(), output);

        sender
            .send(Message::new("sensor", "sensor", json!({ "value": 7 })))
            .await
            .unwrap();

        processor.process(&mut context).await.unwrap();

        let message = output_receiver.recv().await.unwrap();
        assert_eq!(message.payload["sensor"]["value"], json!(7));
    }

    #[tokio::test]
    async fn fusion_nest_by_input_keeps_fast_input_visible_with_slow_input_connected() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_string(), json!("nest_by_input"));
        parameters.insert("join_window_ms".to_string(), json!(0));
        let mut processor =
            FusionStage::new("fusion", fusion_stage_config(Some(parameters))).unwrap();
        let mut context = ProcessingContext::new("fusion".to_string());
        let (fast_sender, fast_receiver) = mpsc::channel(8);
        let (_slow_sender, slow_receiver) = mpsc::channel(8);
        let output = Arc::new(Channel::new(ChannelType::Broadcast, 4));
        let mut output_receiver = output.subscribe();

        context.add_input("hf".to_string(), Subscriber::Mpsc(fast_receiver));
        context.add_input("lf".to_string(), Subscriber::Mpsc(slow_receiver));
        context.attach_output("fusion_data".to_string(), output);

        fast_sender
            .send(Message::new("hf", "hf", json!({ "value": 101 })))
            .await
            .unwrap();

        processor.process(&mut context).await.unwrap();

        let message = output_receiver.recv().await.unwrap();
        assert_eq!(message.payload["hf"]["value"], json!(101));
        assert!(message.payload.get("lf").is_none());
    }

    #[tokio::test]
    async fn fusion_latest_by_input_reuses_slow_stream_values() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_string(), json!("latest_by_input"));
        let mut processor =
            FusionStage::new("fusion", fusion_stage_config(Some(parameters))).unwrap();
        let mut context = ProcessingContext::new("fusion".to_string());
        let (fast_sender, fast_receiver) = mpsc::channel(4);
        let (slow_sender, slow_receiver) = mpsc::channel(4);
        let output = Arc::new(Channel::new(ChannelType::Broadcast, 4));
        let mut output_receiver = output.subscribe();

        context.add_input("fast".to_string(), Subscriber::Mpsc(fast_receiver));
        context.add_input("slow".to_string(), Subscriber::Mpsc(slow_receiver));
        context.attach_output("fusion_data".to_string(), output);

        fast_sender
            .send(Message::new("fast", "fast", json!({ "value": 1 })))
            .await
            .unwrap();
        processor.process(&mut context).await.unwrap();
        assert!(output_receiver.try_recv().await.is_none());

        slow_sender
            .send(Message::new("slow", "slow", json!({ "value": 10 })))
            .await
            .unwrap();
        processor.process(&mut context).await.unwrap();

        let first_fused = output_receiver.recv().await.unwrap();
        assert_eq!(first_fused.payload["fast"]["value"], json!(1));
        assert_eq!(first_fused.payload["slow"]["value"], json!(10));

        fast_sender
            .send(Message::new("fast", "fast", json!({ "value": 2 })))
            .await
            .unwrap();
        processor.process(&mut context).await.unwrap();

        let second_fused = output_receiver.recv().await.unwrap();
        assert_eq!(second_fused.payload["fast"]["value"], json!(2));
        assert_eq!(second_fused.payload["slow"]["value"], json!(10));
    }

    #[test]
    fn fusion_rejects_invalid_mode() {
        let mut parameters = HashMap::new();
        parameters.insert("mode".to_string(), json!("zipper"));

        let error = FusionConfig::from_stage_config(&fusion_stage_config(Some(parameters)))
            .expect_err("invalid fusion mode is rejected");

        assert!(error.to_string().contains("mode"));
    }

    #[test]
    fn fusion_rejects_invalid_join_window_type() {
        let mut parameters = HashMap::new();
        parameters.insert("join_window_ms".to_string(), json!("soon"));

        let error = FusionConfig::from_stage_config(&fusion_stage_config(Some(parameters)))
            .expect_err("invalid join window type is rejected");

        assert!(error.to_string().contains("join_window_ms"));
    }

    fn fusion_stage_config(parameters: Option<HashMap<String, Value>>) -> StageConfig {
        StageConfig {
            r#type: "fusion".to_string(),
            inputs: Some(vec!["a".to_string(), "b".to_string()]),
            output: Some("fusion_data".to_string()),
            concurrency: None,
            channel: None,
            timing: None,
            parameters,
        }
    }
}
