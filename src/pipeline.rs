use crate::aggregation::{Aggregator, Fusion};
use crate::message::Message;
use crate::registry::MessageRegistry;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{error, info};

pub async fn run_aggregation_pipeline(
    registry: MessageRegistry,
    inputs: Vec<String>,
    tx: Sender<Message>,
) {
    let aggregator = Fusion;

    loop {
        let mut messages = Vec::new();

        for input_name in &inputs {
            if let Some(sender) = registry.get(input_name).await {
                if let Ok(message) = sender.recv().await {
                    messages.push(message);
                } else {
                    error!("Failed to receive message from '{}'", input_name);
                }
            }

            if let Some(aggregated) = aggregator.aggregate(messages).await {
                if let Err(e) = tx.send(aggregated).await {
                    error!("Failed to send aggregated message: {}", e);
                }
            }
        }
    }
}
