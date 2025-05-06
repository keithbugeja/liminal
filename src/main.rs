mod config;
mod input;
mod logging;
mod sink;
mod transform;

mod message;

use std::sync::Arc;

use input::create_input_source_handler;
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

    // Start pipeline
    let (tx_shutdown, _) = tokio::sync::broadcast::channel::<()>(16);
    let tx_shutdown = Arc::new(tx_shutdown);

    let (tx_input, rx_input) = tokio::sync::mpsc::channel::<message::Message>(64);
    let (tx_transformed, mut rx_transformed) = tokio::sync::mpsc::channel::<message::Message>(64);
    let mut handles = vec![];

    for (name, input_def) in &config.input_sources {
        let name = name.clone();
        let input_def = input_def.clone();
        let tx = tx_input.clone();
        let shutdown = tx_shutdown.subscribe();

        match create_input_source_handler(&name, &input_def) {
            Ok(handler) => {
                let handle = tokio::spawn(async move {
                    if let Err(e) = handler.run(tx, shutdown).await {
                        error!("Input '{}' failed: {}", name, e);
                    }
                });
                handles.push(handle);
            }
            Err(e) => error!("Failed to create input source handler for {}: {}", name, e),
        }
    }

    let transforms: Vec<Box<dyn transform::Transform>> = config
        .transformations
        .values()
        .map(|t| transform::create_transform(&t.kind, &t.parameters).expect("invalid transform"))
        .collect();

    let tx_sinks = tx_transformed.clone();

    tokio::spawn(async move {
        let mut rx = rx_input;
        while let Some(msg) = rx.recv().await {
            let mut processed = Some(msg);

            for transform in &transforms {
                if let Some(msg) = processed {
                    processed = transform.apply(msg).await;
                } else {
                    break;
                }
            }

            if let Some(msg) = processed {
                if let Err(e) = tx_sinks.send(msg).await {
                    error!("Failed to forward transformed message: {}", e);
                }
            }
        }
    });

    let sinks: Vec<Box<dyn sink::OutputSinkHandler>> = config
        .output_sinks
        .iter()
        .map(|(name, def)| sink::create_output_sink_handler(name, def).expect("invalid sink"))
        .collect();

    tokio::spawn(async move {
        while let Some(msg) = rx_transformed.recv().await {
            for sink in &sinks {
                sink.handle(msg.clone()).await;
            }
        }
    });

    // tokio::spawn(async move {
    //     while let Some(msg) = rx_input.recv().await {
    //         info!(target:"pipeline", "Received message from {}: {:?}", msg.source, msg.payload);
    //     }
    // });

    let tx_shutdown_clone = tx_shutdown.clone();
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl + C: {}", e);
        }
        info!("Received Ctrl+C -> shutting down.");
        let _ = tx_shutdown_clone.send(());
    });

    futures::future::join_all(handles).await;

    info!("All input sources have been processed.");
}
