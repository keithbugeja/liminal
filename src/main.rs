mod config;
mod core;
mod logging;
mod processors;

use config::ChannelType;
use core::channel::*;
use tokio::task;

use tracing::{error, info};

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
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

    let _ = core::pipeline::PipelineManager::new(config)
        .build_all()
        .expect("pipeline building")
        .connect_stages()
        .await
        .expect("pipeline connection")
        .start_all()
        .await
        .expect("pipeline started")
        .wait_for_all_stages()
        .await;

    info!("All input sources have been processed.");
}
