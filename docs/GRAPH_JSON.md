# Resolved Pipeline Graph JSON

The graph JSON is the stable contract between the Rust configuration layer and the
future Tauri/React GUI. It is produced from a parsed Liminal `Config` by resolving
implicit channel references into explicit graph edges.

Generate it with:

```powershell
cargo run -- --graph-json --config config/examples/config_rule_filter.toml
```

## Top-Level Shape

```json
{
  "schema_version": 1,
  "summary": {},
  "nodes": [],
  "edges": [],
  "channels": [],
  "diagnostics": []
}
```

`schema_version` is incremented whenever this contract changes in a way that requires
frontend handling.

`summary` gives the frontend cheap status information without scanning the whole graph:

- `node_count`
- `edge_count`
- `channel_count`
- `diagnostic_count`
- `error_count`
- `warning_count`
- `has_errors`

## Nodes

Each node corresponds to one TOML stage table.

Important fields:

- `id`: stable qualified graph ID, such as `input:sensor`,
  `pipeline:main.stage:filter`, or `output:console`
- `kind`: `input`, `pipeline_stage`, or `output`
- `lane`: `inputs`, `pipeline_stages`, or `outputs`
- `lane_index`: deterministic order within the lane
- `display_name`: short stage name for labels
- `config_path`: TOML-like path for future edit commands
- `pipeline_name`: containing pipeline for pipeline stages
- `processor_type`: configured processor type
- `input_channels`: consumed channel names
- `output_channel`: produced channel name, if any

The first read-only GUI should use `lane` and `lane_index` for the initial three-column
layout.

## Edges

Each edge is materialised from a consumer input channel and its resolved producer.

Important fields:

- `id`: stable edge ID
- `source_node_id`
- `target_node_id`
- `channel_name`
- `target_input_index`: index in the target stage's `inputs = [...]` array

If a channel has multiple producers, no edge is emitted for that ambiguous channel.
The ambiguity is reported as a diagnostic instead.

## Channels

Each channel entry groups the producer and all consumers for a channel name.

Important fields:

- `name`
- `producer_node_ids`
- `consumer_node_ids`
- `channel_type`
- `capacity`

Channel type and capacity come from the producing stage's `channel` config. If no
producer exists, default channel settings are reported so the frontend can still render
the dangling channel consistently.

## Diagnostics

Diagnostics are part of the graph contract, not side-channel logs.

Current kinds:

- `dangling_input_channel`
- `duplicate_channel_producer`
- `orphan_produced_channel`
- `cycle_detected`

Current severities:

- `error`
- `warning`

Diagnostics include:

- `message`
- `channel_name`, when applicable
- `node_ids`

The GUI should use diagnostics for both the side panel and inline node/channel badges.
