mod config;
mod core;
mod logging;
mod processors;

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() {
    // Initialize logging
    logging::init_logging("info");

    // Load configuration
    let config_path = "./config/config.toml";
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
    tracing::info!("Configuration loaded and validated successfully.");

    // Initialize the pipeline manager
    tracing::info!("Initialising pipeline manager...");
    let _ = core::pipeline::PipelineManager::new(config)
        .build_all()
        .expect("pipeline building")
        .connect_stages()
        .await
        .expect("pipeline connection")
        .start_all()
        .await
        .expect("pipeline started")
        .wait_for_all()
        .await;

    // Pipeline terminated
    tracing::info!("All input sources have been processed.");
}
