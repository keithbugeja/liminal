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

## Key Features

- **Declarative pipeline configuration**: Define complex data processing workflows in TOML without writing code
- **Pluggable processor architecture**: Easy to write custom input sources, transforms, and output sinks
- **Comprehensive timing semantics**: Event time, watermarks, sequence tracking, deadlines for real-time processing
- **Multiple channel types**: Choose communication patterns (broadcast, direct, shared, fanout) with configurable backpressure
- **Cross-language integration**: TCP protocol with length-prefixed JSON for connecting external systems

## Architecture

Liminal uses a message-passing architecture where:

- **Messages** carry data with source, topic, payload, and comprehensive timing metadata (event time, ingestion time, sequence IDs, watermarks)
- **Channels** provide communication between stages with four types: broadcast, direct (point-to-point), shared (MPMC), and fanout
- **Processors** transform data and forward to output channels using configurable concurrency
- **Pipelines** compose processors into data processing workflows
- **Configuration** defines the complete system declaratively with timing constraints

This design enables high-throughput, low-latency processing with clear separation of concerns.

## Timing Semantics

Liminal provides comprehensive timing semantics for real-time stream processing:

- **Event Time vs Ingestion Time**: Messages track both when events occurred and when they were received
- **Watermarks**: Automatic watermark generation for handling out-of-order data with three strategies:
  - **Periodic**: Generate watermarks at fixed intervals
  - **Punctuated**: Extract watermarks from special fields in the data stream
  - **Heuristic**: Generate watermarks based on event distribution percentiles
- **Processing Deadlines**: Configurable timeouts for real-time guarantees
- **Sequence Tracking**: Automatic sequence ID assignment for message ordering
- **Jitter Bounds**: Configurable timing constraints for latency-sensitive applications

Configure timing on input sources and transform stages (outputs just sink data):

```toml
[inputs.sensor_data.timing]
event_time_field = "timestamp"          # Extract event time from payload
watermark_strategy = { type = "periodic", interval_ms = 1000 }
max_lateness_ms = 5000                   # Drop messages older than 5s
processing_timeout_ms = 10000            # Processing deadline
jitter_bounds_ms = 100                   # Acceptable timing variation
metrics_enabled = true                   # Collect timing metrics
```

## Channel Types

Liminal supports four communication patterns between processing stages:

- **Broadcast** (default): Fast fan-out messaging where slow consumers may miss messages. Best for real-time data streams where latest data is most important.
- **Direct**: Point-to-point MPSC channel with backpressure. Single producer, single consumer with reliable delivery.
- **Shared**: Multi-consumer MPMC channel with backpressure. Multiple consumers share the message load - each message goes to exactly one consumer.
- **Fanout**: Fan-out using multiple MPSC channels with backpressure. Each consumer gets a copy of every message with reliable delivery.

```toml
[inputs.sensor_data]
type = "simulated"
output = "raw_data"
channel = { type = "broadcast", capacity = 256 }  # or "direct", "shared", "fanout"
```

## TCP Protocol

Liminal's TCP processors use a length-prefixed JSON protocol for cross-language integration:

```
[4-byte big-endian length][JSON payload]
```

This format is compatible with Erlang's `{packet, 4}` option, making it easy to integrate with OTP systems. Both TCP input and output processors support client and server modes:

```toml
# TCP Input (server mode - listens for connections)
[inputs.tcp_data]
type = "tcp_input"
output = "external_data"
parameters = { host = "127.0.0.1", port = 9999, mode = "server" }

# TCP Output (client mode - connects to external system)
[outputs.tcp_sink]
type = "tcp_output"
inputs = ["processed_data"]
parameters = { host = "127.0.0.1", port = 8888, mode = "client" }
```

## Current Processors

Liminal ships with these built-in processors:

### Input Processors
- **MQTT Subscriber** (`mqtt_sub`): Subscribe to MQTT topics and convert messages to internal format
- **TCP Input** (`tcp_input`): Receive JSON messages over TCP with length-prefixed protocol
- **Simulated Data Generator** (`simulated`): Generate synthetic data for testing and development

### Transform Processors  
- **Rule Processor** (`rule`): Apply conditional transformations with mathematical expressions, field manipulation, and routing logic

### Output Processors
- **Console Logger** (`console`): Display processed messages to stdout
- **File Logger** (`file`): Write processed messages to files with configurable formats
- **MQTT Publisher** (`mqtt_pub`): Publish messages to MQTT topics
- **TCP Output** (`tcp_output`): Send JSON messages over TCP with length-prefixed protocol

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

1. **Run with the default configuration**:
```bash
cargo run -- --config config/config.toml
```

2. **See the output**: The default config demonstrates several features:
   - Simulated temperature data with timing semantics
   - Rule-based processing (LED control based on temperature thresholds)
   - Multiple outputs: console logging, MQTT publishing, and TCP integration
   - Processing of MQTT sensor data (if available)

3. **Test TCP integration** (optional):
```bash
# In another terminal, send JSON data to the TCP input
echo '{"test": "data", "value": 42}' | nc 127.0.0.1 9999
```

The default configuration in `config/config.toml` demonstrates:
- Simulated temperature data generation with timing semantics
- MQTT sensor data processing (if broker available)
- TCP input/output for external system integration
- Rule-based processing with mathematical expressions
- Multiple output destinations

For MQTT examples, you can optionally start a broker:
```bash
# Using Docker
docker run -it -p 1883:1883 eclipse-mosquitto
```

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
type = "processor_type"        # simulated, mqtt_sub, tcp_input
output = "channel_name"
concurrency = { type = "thread" }  
channel = { type = "broadcast", capacity = 256 }

[inputs.input_name.parameters]
# processor-specific config here

# Optional timing configuration (inputs and transforms only)
[inputs.input_name.timing]
event_time_field = "timestamp"
watermark_strategy = { type = "periodic", interval_ms = 1000 }
max_lateness_ms = 5000
processing_timeout_ms = 10000
jitter_bounds_ms = 100
metrics_enabled = true

# Processing pipelines
[pipelines.pipeline_name]
description = "Pipeline description"

[pipelines.pipeline_name.stages.stage_name]
type = "rule"                  # currently only rule processor available
inputs = ["input_channel"]
output = "output_channel"
concurrency = { type = "thread" }  
channel = { type = "broadcast", capacity = 256 }

[pipelines.pipeline_name.stages.stage_name.parameters]
# processor-specific config here

# Output sinks (no timing config - they just sink data)
[outputs.output_name]
type = "processor_type"        # console, file, mqtt_pub, tcp_output
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

### Rule Actions

The rule processor supports these actions for data transformation:

```toml
[[pipelines.stage_name.parameters.rules]]
condition = { field_path = "temperature", operation = ">", value = 25.0 }
actions = [
    { type = "set_field", field_path = "status", value = "hot" },
    { type = "compute_field", field_path = "temp_f", expression = "temperature * 9/5 + 32" },
    { type = "copy_field", source_field = "device", target_field = "sensor_id" },
    { type = "rename_field", old_field = "temp", new_field = "temperature" },
    { type = "remove_field", field_path = "debug_info" },
    { type = "keep_only_fields", field_paths = ["temperature", "status"] },
    { type = "pass_through" }
]
else_actions = [
    { type = "drop_message" }
]
```

Available actions:
- **set_field**: Set a field to a constant value
- **compute_field**: Calculate field using mathematical expressions
- **copy_field**: Copy value from one field to another
- **rename_field**: Rename a field
- **remove_field**: Remove a field from the message
- **keep_only_fields**: Keep only specified fields, remove all others
- **pass_through**: Let the message pass unchanged
- **drop_message**: Drop the message entirely

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