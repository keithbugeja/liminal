#![allow(dead_code)]

use clap::Parser;

mod config;
mod core;
mod logging;
mod processors;

/// Liminal - A framework for building data processing pipelines
#[derive(Parser)]
#[command(name = "liminal")]
#[command(author = "Keith Bugeja <keith.bugeja@um.edu.mt>")]
#[command(version = "0.2.0")]
#[command(about = "Liminal: A Zero-Code Stream Processing Engine for Sensor Data")]
#[command(long_about =
"------------------------------------------------------------
    ██╗     ██╗███╗   ███╗██╗███╗   ██╗ █████╗ ██╗     
    ██║     ██║████╗ ████║██║████╗  ██║██╔══██╗██║     
    ██║     ██║██╔████╔██║██║██╔██╗ ██║███████║██║     
    ██║     ██║██║╚██╔╝██║██║██║╚██╗██║██╔══██║██║     
    ███████╗██║██║ ╚═╝ ██║██║██║ ╚████║██║  ██║███████╗
    ╚══════╝╚═╝╚═╝     ╚═╝╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝╚══════╝                                                        

    Stream processing engine for sensor data. Build real-
    time pipelines using TOML configuration files.
------------------------------------------------------------")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "./config/config.toml")]
    config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// List available processor types
    #[arg(short = 'L', long)]
    list_processors: bool,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 32)]
async fn main() {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logging with specified level
    logging::init_logging(&cli.log_level);

    // Handle list processors command
    if cli.list_processors {
        println!("Available processor types:");
        let processors = processors::factory::list_processors();
        for processor in processors {
            println!("  - {}", processor);
        }
        return;
    }

    // Load configuration from specified file
    let config = match config::load_config(&cli.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to load config from '{}': {}", cli.config, e);
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
