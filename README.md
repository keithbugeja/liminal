# Liminal

```
    ██╗     ██╗███╗   ███╗██╗███╗   ██╗ █████╗ ██╗     
    ██║     ██║████╗ ████║██║████╗  ██║██╔══██╗██║     
    ██║     ██║██╔████╔██║██║██╔██╗ ██║███████║██║     
    ██║     ██║██║╚██╔╝██║██║██║╚██╗██║██╔══██║██║     
    ███████╗██║██║ ╚═╝ ██║██║██║ ╚████║██║  ██║███████╗
    ╚══════╝╚═╝╚═╝     ╚═╝╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝╚══════╝                     
```

A high-performance, configurable data processing pipeline framework written in Rust. Liminal enables real-time data transformation, filtering, and routing through a modular stage-based architecture that you configure with TOML files.

## Goals

- **Real-time Processing**: Handle continuous data streams with minimal latency
- **Modular Architecture**: Compose complex pipelines from reusable processing stages
- **High Performance**: Leverage Rust's performance and safety for data-intensive workloads
- **Flexible Configuration**: Define pipelines declaratively using TOML configuration
- **Extensible**: Easy-to-add custom processors for domain-specific needs

## Architecture

Liminal uses a message-passing architecture where:

- **Messages** carry data with source, topic, payload, and timestamp
- **Channels** provide communication between stages (broadcast/MPSC)
- **Processors** transform data and forward to output channels
- **Pipelines** compose processors into data processing workflows
- **Configuration** defines the complete system declaratively

This design enables high-throughput, low-latency processing with clear separation of concerns.

## Current Processors

Liminal ships with these built-in processors:

### Input Processors
- **MQTT Subscriber** (`mqtt_sub`): Subscribe to MQTT topics and convert messages to internal format
- **Simulated Data Generator** (`simulated`): Generate synthetic data for testing and development

### Transform Processors  
- **Rule Processor** (`rule`): Apply conditional transformations with mathematical expressions, field manipulation, and routing logic

### Output Processors
- **Console Logger** (`console`): Display processed messages to stdout
- **File Logger** (`file`): Sink processed messages to file
- **MQTT Publisher** (`mqtt_pub`): Publish messages to MQTT topics

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

2. **Configure a pipeline** in `config/config.toml` (or try the examples in `config/examples/`):
```toml
# Input: Subscribe to MQTT sensor data
[inputs.mqtt_sensors]
type = "mqtt_sub"
output = "all_sensor_data"
channel = { type = "broadcast", capacity = 1000 }
parameters = { 
    broker_url = "mqtt://localhost:1883", 
    client_id = "liminal_rule_test",
    topics = ["liminal/sensors/+/+"]
}

# Pipeline: Process and enrich sensor data  
[pipelines.mqtt_pipeline]
description = "Processes MQTT sensor data with rules"

[pipelines.mqtt_pipeline.stages.rule_enricher]
type = "rule"
inputs = ["all_sensor_data"]
output = "enriched_data"
channel = { type = "broadcast", capacity = 500 }

# Add device metadata for ESP32 devices
[[pipelines.mqtt_pipeline.stages.rule_enricher.parameters.rules]]
condition = { field_path = "device_id", operation = "startswith", value = "esp32-" }
actions = [
    { type = "set_field", field_path = "device_type", value = "esp32" },
    { type = "copy_field", source_field = "device_id", target_field = "original_device_id" }
]

# Compute acceleration magnitude from IMU data
[[pipelines.mqtt_pipeline.stages.rule_enricher.parameters.rules]]
condition = { field_path = "sensor_type", operation = "equals", value = "imu" }
actions = [
    { type = "compute_field", field_path = "acceleration_magnitude", 
      expression = "sqrt(accelerometer.x^2 + accelerometer.y^2 + accelerometer.z^2)" }
]

# Output: Display enriched data
[outputs.enriched_data_log]
type = "console"
inputs = ["enriched_data"]
```

3. **Run Liminal**:
```bash
cargo run
```

4. **Send test data**:
```bash
# Send sample IMU data to see the rule processor in action
mosquitto_pub -h localhost -t liminal/sensors/esp32-001/imu -m '{
  "device_id": "esp32-001",
  "sensor_type": "imu", 
  "accelerometer": {"x": 0.6, "y": 0.7, "z": 0.3},
  "gyroscope": {"x": 3.5, "y": 0.7, "z": -0.6},
  "temperature": 26.8
}'
```

You should see enriched output with computed `acceleration_magnitude` and added metadata fields.

## Example Configurations

The `config/examples/` directory contains several working examples:

- `config_rule_test.toml` - Basic rule processing with device metadata and sensor math
- `config_led_control_with_else.toml` - LED control using else_actions for binary states
- `config_advanced_rule.toml` - Complex filtering and transformation chains

Run any example with:
```bash
cargo run -- --config config/examples/config_rule_test.toml
```

## Rule Processor Examples

The rule processor is Liminal's most powerful built-in processor. It applies conditional logic to transform, filter, and route messages. Here are some patterns from the example configurations:

### Conditional Field Manipulation

```toml
# From config/examples/config_rule_test.toml
[[rules]]
condition = { field_path = "sensor_type", operation = "equals", value = "imu" }
actions = [
    # Compute a derived field using mathematical expressions
    { type = "compute_field", field_path = "acceleration_magnitude", 
      expression = "sqrt(accelerometer.x^2 + accelerometer.y^2 + accelerometer.z^2)" }
]
else_actions = [
    { type = "drop_message" }  # Drop non-IMU messages
]
```

### Data Minimisation with Pre-computation

```toml
# From config/config.toml - compute magnitude, then keep only essential fields
[[rules]]
condition = { field_path = "sensor_type", operation = "equals", value = "imu" }
actions = [
    # First: compute field using full original payload
    { type = "compute_field", field_path = "acceleration_magnitude", 
      expression = "sqrt(accelerometer.x^2 + accelerometer.y^2 + accelerometer.z^2)" },
    # Then: destructive reset to minimal payload
    { type = "keep_only_fields", field_paths = ["device_id", "acceleration_magnitude"] }
]
```

### Temperature-based Control Logic

```toml
# From config/config.toml - LED control based on temperature
[[rules]]
condition = { field_path = "temperature", operation = ">", value = 15.0 }
actions = [
    { type = "keep_only_fields", field_paths = [] },  # Clear everything
    { type = "set_field", field_path = "state", value = true }
]
else_actions = [
    { type = "keep_only_fields", field_paths = [] },
    { type = "set_field", field_path = "state", value = false }
]
```

### Available Rule Actions

- `set_field`: Add or modify a field with a static value
- `copy_field`: Copy a field to a new location  
- `rename_field`: Rename an existing field
- `remove_field`: Delete a field
- `compute_field`: Calculate a new field using mathematical expressions
- `keep_only_fields`: Reset message to contain only specified fields (destructive)
- `pass_through`: Forward message unchanged
- `drop_message`: Discard the message entirely

### Condition Operations

- `equals`, `not_equals`: Exact value matching
- `startswith`, `endswith`, `contains`: String pattern matching
- `>`, `>=`, `<`, `<=`: Numerical comparisons

### Mathematical Expressions

Computed fields support standard mathematical operations and functions:
- Basic operators: `+`, `-`, `*`, `/`, `^` (power)
- Functions: `sqrt()`, `sin()`, `cos()`, `tan()`, `abs()`, `ln()`, `log()`, `max()`, `min()`
- Field references: Use dot notation like `accelerometer.x`, `sensor.temperature`

## Configuration Structure

Liminal uses TOML configuration files to define processing pipelines. The main configuration file is `config/config.toml`.

### Basic Configuration Pattern

```toml
# Input sources
[inputs.input_name]
type = "processor_type"
output = "channel_name"
concurrency = { type = "thread" }  
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

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` and `cargo clippy` pass
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Related Projects

- **Liminal Firmware**: [Firmware for various sensors and microcontrollers that provide data to **Liminal**](https://github.com/keithbugeja/liminal-firmware) 

---

For questions or support, please open an issue in this repository.