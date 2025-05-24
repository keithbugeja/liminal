# Liminal

A flexible data processing pipeline framework built in Rust for real-time stream processing. Liminal lets you build complex data flows using simple configuration files, handling everything from data ingestion to transformation and output.

## What it does

Liminal processes data streams through configurable pipelines. You define your data flow in TOML configuration files, and Liminal handles the rest - message routing, backpressure, error handling, and concurrent processing.

Think of it like connecting data processing blocks together:
- **Input processors** generate or collect data (simulated sensors, file readers, APIs, MQTT brokers)
- **Transform processors** modify data as it flows through (scaling, filtering, aggregation)  
- **Output processors** send results somewhere useful (files, databases, dashboards, MQTT publishers)

Pipelines can run locally or be distributed across multiple machines using socket-based communication (in progress), letting you scale processing horizontally and connect remote data sources.

## Quick start

Here's a simple pipeline that generates sensor data, scales it, and logs the results:

```toml
# config.toml
[inputs.temperature_sensor]
type = "simulated"
output = "raw_temp"
parameters = { field_out = "temperature", interval_ms = 1000, min_value = 15.0, max_value = 35.0 }

[pipelines.processing.stages.scale_temp]
type = "scale"
inputs = ["raw_temp"]
output = "scaled_temp"
parameters = { field_in = "temperature", field_out = "temp_fahrenheit", scale_factor = 1.8 }

[outputs.console]
type = "console"
inputs = ["scaled_temp"]
```

Run it with:
```bash
cargo run -- config.toml
```

You'll see temperature readings converted from Celsius to Fahrenheit streaming to your console.

## Core concepts

### Stages and pipelines

Data flows through **stages** connected by **channels**. Each stage runs independently and processes messages at its own pace.

- **Input stages** create data (sensors, file readers, network sources, MQTT subscribers)
- **Pipeline stages** transform data (math operations, filtering, aggregation)
- **Output stages** consume data (logging, file writing, network publishing, MQTT publishers)

### Distributed processing

Liminal will support distributed processing by connecting pipelines across multiple machines using socket-based input/output endpoints. This lets you:

- Scale processing across multiple nodes
- Connect remote data sources and sinks
- Build fault-tolerant distributed systems
- Separate data collection, processing, and storage

```toml
# Machine A: Data collection
[inputs.sensor_farm]
type = "mqtt"
parameters = { broker = "mqtt://sensors.local", topic = "farm/+/data" }

[outputs.processing_node]
type = "socket"
inputs = ["sensor_farm"]
parameters = { endpoint = "tcp://processing.local:9001" }

# Machine B: Data processing  
[inputs.from_collectors]
type = "socket"
parameters = { bind = "tcp://0.0.0.0:9001" }

[pipelines.analytics.stages.aggregate]
type = "window_aggregate"
inputs = ["from_collectors"]
output = "aggregated"

[outputs.storage_node]
type = "socket"
inputs = ["aggregated"]
parameters = { endpoint = "tcp://storage.local:9002" }
```

### Messages

Everything flows as JSON messages with this structure:
```json
{
  "source": "temperature_sensor",
  "topic": "raw_temp", 
  "payload": {"temperature": 23.5},
  "timestamp": 1640995200000
}
```

### Channels

Stages communicate through channels that handle different delivery patterns:
- **Broadcast**: Fast fan-out, may drop messages for slow consumers
- **Direct**: Reliable point-to-point delivery
- **Shared**: Load balancing across multiple consumers
- **Fanout**: Reliable delivery to all subscribers

## Configuration

Liminal uses TOML configuration files with three main sections:

### Inputs
```toml
[inputs.sensor_data]
type = "simulated"
output = "raw_readings"
parameters = { 
  field_out = "value",
  distribution = "normal",
  min_value = 0.0,
  max_value = 100.0,
  interval_ms = 500
}

# MQTT input (planned)
[inputs.iot_devices]
type = "mqtt"
output = "device_data"
parameters = {
  broker = "mqtt://broker.local:1883",
  topic = "devices/+/telemetry",
  qos = 1
}

# Socket input for distributed processing (planned)
[inputs.remote_data]
type = "socket"
output = "network_stream"
parameters = {
  bind = "tcp://0.0.0.0:9000",
  format = "json"
}
```

### Pipelines
```toml
[pipelines.data_processing.stages.filter]
type = "lowpass"
inputs = ["raw_readings"]
output = "filtered_data"
parameters = { 
  field_in = "value", 
  field_out = "filtered_value",
  threshold = 50.0
}
```

### Outputs
```toml
[outputs.file_logger]
type = "file"
inputs = ["filtered_data"]
parameters = {
  file_path = "logs/output.jsonl",
  format = "json",
  append = true
}

# MQTT output (planned)
[outputs.telemetry_publisher]
type = "mqtt"
inputs = ["processed_data"]
parameters = {
  broker = "mqtt://telemetry.local:1883",
  topic_template = "results/{{source}}/{{field}}",
  qos = 1,
  retain = false
}

# Socket output for distributed processing (planned)
[outputs.next_stage]
type = "socket"
inputs = ["filtered_data"]
parameters = {
  endpoint = "tcp://analytics.local:9001",
  format = "json"
}
```

## Built-in processors

### Input processors
- **simulated**: Generate synthetic data with configurable distributions
- **file**: Read data from files (planned)
- **http**: HTTP endpoint data source (planned)
- **mqtt**: Subscribe to MQTT topics (planned)
- **socket**: Receive data over TCP/UDP sockets (planned)

### Transform processors  
- **scale**: Multiply values by a constant factor
- **lowpass**: Filter values below a threshold
- **aggregate**: Combine multiple values (planned)
- **window**: Time-based windowing operations (planned)

### Output processors
- **console**: Print messages to stdout
- **file**: Write to files in JSON, CSV, or text format
- **http**: POST to HTTP endpoints (planned)
- **mqtt**: Publish to MQTT topics (planned)
- **socket**: Send data over TCP/UDP sockets (planned)

## Distributed architecture patterns

### Edge-to-cloud processing
```toml
# Edge device: Collect and preprocess
[inputs.local_sensors]
type = "simulated"
parameters = { interval_ms = 100 }

[pipelines.edge.stages.smooth]
type = "moving_average"
inputs = ["local_sensors"]
output = "smoothed"

[outputs.to_cloud]
type = "socket"
inputs = ["smoothed"]
parameters = { endpoint = "tcp://cloud.example.com:9000" }

# Cloud: Analytics and storage
[inputs.from_edge]
type = "socket"
parameters = { bind = "tcp://0.0.0.0:9000" }

[pipelines.analytics.stages.detect_anomalies]
type = "anomaly_detector"
inputs = ["from_edge"]
output = "alerts"
```

### Fan-out processing
```toml
# Central collector
[inputs.data_source]
type = "mqtt"
parameters = { broker = "mqtt://central", topic = "data/+" }

# Distribute to specialized processors
[outputs.processor_a]
type = "socket"
inputs = ["data_source"]
parameters = { endpoint = "tcp://proc-a:9000" }

[outputs.processor_b] 
type = "socket"
inputs = ["data_source"]
parameters = { endpoint = "tcp://proc-b:9000" }
```

## Field mapping

Processors use flexible field mapping to work with your data structure:

```toml
# Single field transformation
parameters = { field_in = "temperature", field_out = "temp_celsius" }

# Multiple parallel fields  
parameters = { fields_in = ["temp", "humidity"], fields_out = ["temp_c", "humidity_pct"] }

# Custom mapping
parameters = { field_mapping = { "sensor_1" = "temperature", "sensor_2" = "pressure" } }
```

## Building and running

```bash
# Development
cargo run -- config.toml

# Release build
cargo build --release
./target/release/liminal config.toml

# With logging
RUST_LOG=info cargo run -- config.toml

# Distributed setup
# Terminal 1: Data collector
./liminal collector.toml

# Terminal 2: Data processor  
./liminal processor.toml

# Terminal 3: Data storage
./liminal storage.toml
```

## Examples

Check the `config/` directory for example configurations:

- `config.toml`: Basic sensor simulation with scaling and filtering
- `multi-pipeline.toml`: Multiple parallel processing chains
- `file-output.toml`: Writing results to various file formats (planned)
- `distributed.toml`: Multi-node processing setup (planned)
- `mqtt-bridge.toml`: MQTT integration examples (planned)

## Architecture

Liminal is built around a few key principles:

**Async-first**: Everything uses async/await for efficient I/O
**Type-safe configuration**: TOML configs are validated at startup
**Backpressure handling**: Slow consumers don't break the whole pipeline
**Distributed-ready**: Socket-based endpoints for multi-node deployment
**Protocol-agnostic**: Support for various transport protocols (TCP, UDP, MQTT)
**Extensible**: Easy to add new processor types
**Observable**: Built-in logging and metrics (coming soon)

The core loop is simple:
1. Load and validate configuration
2. Create channels and wire up stages  
3. Start all processors concurrently
4. Handle messages flowing between stages
5. Gracefully shutdown on signals

For distributed setups, socket endpoints handle serialization, network transport, and reconnection automatically.

## Adding custom processors

Create a new processor by implementing the `Processor` trait:

```rust
pub struct MyProcessor {
    name: String,
    config: MyConfig,
}

#[async_trait]
impl Processor for MyProcessor {
    async fn init(&mut self) -> anyhow::Result<()> {
        // Setup code
    }
    
    async fn process(&mut self, context: &mut ProcessingContext) -> anyhow::Result<()> {
        // Process incoming messages
        for (channel_name, input) in context.inputs.iter_mut() {
            while let Some(message) = input.try_recv().await {
                // Transform the message
                let result = transform(message.payload);
                
                // Send to output
                if let Some(output) = &context.output {
                    output.channel.publish(result).await?;
                }
            }
        }
        Ok(())
    }
}
```

Register it in `src/processors/mod.rs` and you're ready to use it in configs.

## Roadmap

**Network protocols**
- MQTT input/output processors for IoT integration
- TCP/UDP socket processors for custom protocols
- HTTP endpoints for REST API integration

**Distributed processing**  
- Automatic service discovery and load balancing
- Fault tolerance and automatic failover
- Configuration synchronization across nodes

**Advanced processing**
- Time-based windowing and aggregation
- Stream joins and complex event processing
- Machine learning inference integration

**Operations**
- Built-in metrics and health monitoring
- Configuration hot-reloading
- Performance profiling and optimization tools

## License

MIT License - see LICENSE file for details.

## Contributing

This is an experimental project exploring real-time data processing patterns in Rust. Ideas, feedback, and contributions welcome!

Areas that could use help:
- MQTT and socket-based input/output processors
- Distributed coordination and service discovery
- Advanced transformation processors (windowing, aggregation, ML)
- Performance optimization and benchmarking
- Better error handling and recovery
- Metrics and monitoring integration