mod config;
mod core;
mod logging;
mod stages;

use config::ChannelType;
use core::channel::*;
use stages::factory;
use tokio::task;

use tracing::{error, info};

#[tokio::main]
async fn main() {
    logging::init_logging("info");

    // Load configuration
    let config_path = "/Users/keith/Development/liminal/config/config.toml";
    let config = match config::load_config(config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config::validate_config(&config) {
        tracing::error!("Configuration error: {e}");
        std::process::exit(1);
    }

    // Configuration loaded and validated
    tracing::info!("Configuration loaded and validated.");

    // let mut ch = Channel::<core::message::Message>::new(ChannelType::Broadcast, 16);
    // let mut ch = Channel::<core::message::Message>::new(ChannelType::Flume, 32);
    let mut ch = Channel::<core::message::Message>::new(ChannelType::Fanout, 32);
    let mut a = ch.subscribe();
    let mut b = ch.subscribe();

    let payload = serde_json::json!({ "user": "alice", "action": "login" });
    let msg = core::message::Message::new("auth-service", "user-events", payload);

    ch.publish(msg.clone()).await.unwrap();
    println!("Published message: {:?}", msg);

    task::spawn(async move {
        while let Some(m) = b.recv().await {
            info!("[B] Received message: {:?}", m);
        }
    });

    task::spawn(async move {
        while let Some(m) = a.recv().await {
            info!("[A] Received message: {:?}", m);
        }
    });

    core::pipeline::PipelineManager::new(config)
        .build_all()
        .expect("pipeline building")
        .start_all()
        .await;

    info!("All input sources have been processed.");
}
