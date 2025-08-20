# Liminal

A high-performance, configurable data processing pipeline framework written in Rust. Liminal enables real-time data transformation, filtering, and routing through a modular stage-based architecture.

## Goals

- **Real-time Processing**: Handle continuous data streams with minimal latency
- **Modular Architecture**: Compose complex pipelines from reusable processing stages
- **High Performance**: Leverage Rust's performance and safety for data-intensive workloads
- **Flexible Configuration**: Define pipelines declaratively using TOML configuration
- **Extensible**: Easy-to-add custom processors for domain-specific needs

## Features

### Input Processors
- **MQTT Input**: Subscribe to MQTT topics for IoT data ingestion
- **Simulated Input**: Generate synthetic data for testing and development

### Transform Processors
- **Low-pass Filter**: Apply threshold-based filtering to numeric data
- **Scale Transform**: Scale numeric values by configurable factors
- **Rename Transform**: Map and rename fields in data messages
- **Fusion Aggregator**: Combine multiple data streams

### Output Processors
- **Console Output**: Display processed data to stdout
- **File Output**: Write data to files with configurable formatting
- **MQTT Output**: Publish processed data to MQTT topics

### Core Features
- **Multi-threading**: Configurable concurrency per stage
- **Channel Types**: Broadcast and MPSC channels for different communication patterns
- **Field Mapping**: Flexible field transformation and renaming
- **Error Handling**: Robust error propagation and logging
- **Hot Configuration**: Declarative pipeline configuration via TOML

## Quick Start

### Prerequisites

- Rust 1.70 or later
- (Optional) Docker for MQTT broker testing

### Installation

```bash
git clone https://github.com/keithbugeja/liminal.git
cd liminal
cargo build --release
```

### Running Your First Pipeline

1. **Start an MQTT broker** (for MQTT examples):
```bash
# Using Docker
docker run -it -p 1883:1883 eclipse-mosquitto

# Or using Homebrew on macOS
brew install mosquitto
mosquitto -v
```

2. **Configure a pipeline** in `config/config.toml`:
```toml
[inputs.mqtt_sensor]
type = "mqtt_sub"
output = "raw_sensor_data"
concurrency = { type = "thread" }
channel = { type = "broadcast", capacity = 256 }

[inputs.mqtt_sensor.parameters]
broker_url = "mqtt://localhost:1883"
topics = ["sensors/temperature"]
client_id = "liminal_input"
qos = 0

[pipelines.sensor_pipeline]
description = "Process sensor data"

[pipelines.sensor_pipeline.stages.filter_values]
type = "lowpass"
inputs = ["raw_sensor_data"]
output = "filtered_data"

[pipelines.sensor_pipeline.stages.filter_values.parameters]
field_in = "temperature"
field_out = "filtered_temperature"
threshold = 0.0

[pipelines.sensor_pipeline.stages.console_out]
type = "console"
inputs = ["filtered_data"]
```

3. **Run Liminal**:
```bash
cargo run
```

4. **Send test data**:
```bash
mosquitto_pub -h localhost -t sensors/temperature -m '{"temperature": 23.5}'
```

## Configuration

Liminal uses TOML configuration files to define processing pipelines. The main configuration file is `config/config.toml`.

### Configuration Structure

```toml
# Input sources
[inputs.input_name]
type = "processor_type"
output = "channel_name"
concurrency = { type = "thread" }  # or "async"
channel = { type = "broadcast", capacity = 256 }

[inputs.input_name.parameters]
# processor-specific config here

# Processing pipelines
[pipelines.pipeline_name]
description = "Pipeline description"

[pipelines.pipeline_name.stages.stage_name]
type = "processor_type"
inputs = ["input_channel"]
output = "output_channel"

[pipelines.pipeline_name.stages.stage_name.parameters]
# processor-specific config here

# Output sinks
[outputs.output_name]
type = "processor_type"
inputs = ["input_channel"]

[outputs.output_name.parameters]
# processor-specific config here
```

### Field Configuration

Most processors support field mapping for data transformation:

```toml
# Single field mapping
[stage.parameters]
field_in = "raw_temp"
field_out = "temperature"

# Multiple field mapping
[stage.parameters]
fields_in = ["temp", "humidity"]
fields_out = ["temperature", "humidity_pct"]
```

## Adding Custom Processors

### 1. Create a New Processor

```rust
use crate::processors::Processor;
use crate::config::{ProcessorConfig, StageConfig};
use crate::core::{ProcessingContext, Message};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct MyProcessorConfig {
    // Your configuration fields
}

impl ProcessorConfig for MyProcessorConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self> {
        // Parse configuration from TOML
        Ok(Self { /* ... */ })
    }

    fn validate(&self) -> anyhow::Result<()> {
        // Validate configuration
        Ok(())
    }
}

pub struct MyProcessor {
    name: String,
    config: MyProcessorConfig,
}

impl MyProcessor {
    pub fn new(name: &str, config: StageConfig) -> anyhow::Result<Box<dyn Processor>> {
        let processor_config = MyProcessorConfig::from_stage_config(&config)?;
        processor_config.validate()?;
        
        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
        }))
    }
}

#[async_trait]
impl Processor for MyProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        tracing::info!("MyProcessor '{}' initialised", self.name);
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Your processing logic here
        Ok(())
    }
}
```

### 2. Register the Processor

Add your processor to the factory in `src/processors/factory.rs`:

```rust
fn ensure_default_processors() {
    static INITIALIZED: OnceLock<()> = OnceLock::new();
    INITIALIZED.get_or_init(|| {
        // ... existing processors ...
        register_processor("my_processor", Box::new(MyProcessor::new));
    });
}
```

### 3. Add to Module

Update the appropriate module file (`src/processors/input/mod.rs`, `src/processors/transform/mod.rs`, or `src/processors/output/mod.rs`):

```rust
pub mod my_processor;
pub use my_processor::MyProcessor;
```

## Development

### Running Tests

```bash
cargo test
```

### Generating Test Data

Use the included message generator for MQTT testing:

```bash
chmod +x sim/message_gen_mpu6500.sh
./sim/message_gen_mpu6500.sh
```

### Debugging

Set logging level to debug for detailed output:

```rust
logging::init_logging("debug");
```

## Examples

See the `config/examples/` directory for complete configuration examples:

- `config_mqtt_sim.toml`: MQTT input with simulated data processing

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` and `cargo clippy` pass
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Architecture

Liminal uses a message-passing architecture where:

- **Messages** carry data with source, topic, payload, and timestamp
- **Channels** provide communication between stages (broadcast/MPSC)
- **Processors** transform data and forward to output channels
- **Pipelines** compose processors into data processing workflows
- **Configuration** defines the complete system declaratively

This design enables high-throughput, low-latency processing with clear separation of concerns.

## Related Projects

- **Liminal Firmware**: [Firmware for various sensors and microcontrollers that provide data to **Liminal**](https://github.com/keithbugeja/liminal-firmware) 

---

For questions or support, please open an issue in this repository.