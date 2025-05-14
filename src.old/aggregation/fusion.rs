use crate::aggregation::Aggregator;
use crate::config::extract_param;
use crate::core::message::Message;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use tracing::info;

pub struct FusionAggregator {
    method: String,
    weight_a: f64,
    weight_b: f64,
}

impl FusionAggregator {
    pub fn new(params: &Option<HashMap<String, serde_json::Value>>) -> Self {
        let method = extract_param(params, "method", "weighted_average".to_string());
        let weight_a = extract_param(params, "weight_a", 0.5);
        let weight_b = extract_param(params, "weight_b", 0.5);

        Self {
            method,
            weight_a,
            weight_b,
        }
    }

    fn weighted_average(&self, values: Vec<f64>) -> f64 {
        if values.len() != 2 {
            return 0.0;
        }

        self.weight_a * values[0] + self.weight_b * values[1]
    }

    fn simple_average(&self, values: Vec<f64>) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

#[async_trait]
impl Aggregator for FusionAggregator {
    async fn aggregate(&self, inputs: Vec<Message>) -> Option<Message> {
        if inputs.len() < 2 {
            return None;
        }

        let values: Vec<f64> = inputs
            .iter()
            .filter_map(|msg| msg.payload.as_f64())
            .collect();

        if values.len() != 2 {
            return None;
        }

        let result = match self.method.as_str() {
            "weighted_average" => self.weighted_average(values),
            "simple_average" => self.simple_average(values),
            _ => 0.0,
        };

        let combined = json!({ "fused_value": result });

        info!(
            "Fusion aggregation: method = '{}', result = {}",
            self.method, result
        );

        Some(Message {
            source: "fusion".to_string(),
            topic: "aggregated".to_string(),
            payload: combined,
            timestamp: inputs[0].timestamp,
        })
    }
}
