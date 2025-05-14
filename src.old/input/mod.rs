use crate::config::InputSource;
use crate::core::message::Message;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;

pub mod mqtt;
pub mod simulated;

#[async_trait]
pub trait InputSourceHandler: Send + Sync {
    async fn run(
        &self,
        tx: Sender<Message>,
        mut shutdown: Receiver<()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub fn create_input_source(
    name: &str,
    config: &InputSource,
) -> Result<Box<dyn InputSourceHandler>, String> {
    match config.kind.as_str() {
        "mqtt" => Ok(Box::new(mqtt::MqttInputSource::new(name, config)?)),
        "simulated" => Ok(Box::new(simulated::SimulatedInputSource::new(
            name, config,
        )?)),
        _ => Err(format!("Unsupported input source: {}", config.kind)),
    }
}
