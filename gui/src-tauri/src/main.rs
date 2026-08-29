use liminal::config::{load_config, Config, ResolvedPipelineGraph};
use liminal::processors::descriptor::{processor_descriptors, ProcessorDescriptor};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Value};

#[tauri::command]
fn load_graph(path: String) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let config = load_config(&resolved_path).map_err(|error| {
        format!(
            "Failed to load '{}': {}",
            resolved_path.display(),
            error
        )
    })?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn update_node_parameter(
    path: String,
    node_id: String,
    parameter_key: String,
    value: String,
) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    update_existing_parameter(&mut document, &node_id, &parameter_key, &value)?;
    let next_content = document.to_string();
    let config = toml::from_str(&next_content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    fs::write(&resolved_path, next_content)
        .map_err(|error| format!("Failed to write '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn update_node_parameter_json(
    path: String,
    node_id: String,
    parameter_key: String,
    value_json: String,
) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let value = serde_json::from_str::<serde_json::Value>(&value_json)
        .map_err(|error| format!("Failed to parse parameter JSON: {}", error))?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    update_json_parameter(&mut document, &node_id, &parameter_key, &value)?;
    let next_content = document.to_string();
    let config = toml::from_str(&next_content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    fs::write(&resolved_path, next_content)
        .map_err(|error| format!("Failed to write '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn connect_nodes(
    path: String,
    source_node_id: String,
    target_node_id: String,
) -> Result<ResolvedPipelineGraph, String> {
    if source_node_id == target_node_id {
        return Err("A node cannot be connected to itself".to_string());
    }

    if target_node_id.starts_with("input:") {
        return Err("Input stages cannot consume channels".to_string());
    }

    let resolved_path = resolve_config_path(&path)?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let config = toml::from_str::<Config>(&content)
        .map_err(|error| format!("Failed to parse config before rewiring: {}", error))?;
    let graph = ResolvedPipelineGraph::from_config(&config);
    let source_node = graph
        .nodes
        .iter()
        .find(|node| node.id == source_node_id)
        .ok_or_else(|| format!("Source node '{}' not found", source_node_id))?;
    let target_node = graph
        .nodes
        .iter()
        .find(|node| node.id == target_node_id)
        .ok_or_else(|| format!("Target node '{}' not found", target_node_id))?;
    let channel_name = source_node.output_channel.as_ref().ok_or_else(|| {
        format!(
            "Source node '{}' does not produce an output channel",
            source_node.display_name
        )
    })?;

    if target_node.input_channels.iter().any(|input| input == channel_name) {
        return Err(format!(
            "Target node '{}' already consumes channel '{}'",
            target_node.display_name, channel_name
        ));
    }

    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    add_input_channel(&mut document, &target_node_id, channel_name)?;
    write_document_and_resolve(&resolved_path, document)
}

#[tauri::command]
fn disconnect_edge(
    path: String,
    target_node_id: String,
    channel_name: String,
) -> Result<ResolvedPipelineGraph, String> {
    if target_node_id.starts_with("input:") {
        return Err("Input stages do not have input channel lists".to_string());
    }

    let resolved_path = resolve_config_path(&path)?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    remove_input_channel(&mut document, &target_node_id, &channel_name)?;
    write_document_and_resolve(&resolved_path, document)
}

#[tauri::command]
fn update_node_field(
    path: String,
    node_id: String,
    field_key: String,
    value: String,
) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    update_existing_field(&mut document, &node_id, &field_key, &value)?;
    let next_content = document.to_string();
    let config = toml::from_str(&next_content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    fs::write(&resolved_path, next_content)
        .map_err(|error| format!("Failed to write '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn list_example_configs() -> Result<Vec<String>, String> {
    let examples_dir = repo_root().join("config").join("examples");
    let mut configs = vec!["config/config.toml".to_string()];

    for entry in fs::read_dir(&examples_dir)
        .map_err(|error| format!("Failed to read '{}': {}", examples_dir.display(), error))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            configs.push(relative_to_repo(&path));
        }
    }

    configs.sort();
    Ok(configs)
}

#[tauri::command]
fn list_processor_descriptors() -> Vec<ProcessorDescriptor> {
    processor_descriptors()
}

fn resolve_config_path(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);

    if candidate.is_absolute() && candidate.exists() {
        return Ok(candidate);
    }

    let cwd_candidate = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(path);
    if cwd_candidate.exists() {
        return Ok(cwd_candidate);
    }

    let repo_candidate = repo_root().join(path);
    if repo_candidate.exists() {
        return Ok(repo_candidate);
    }

    Err(format!("Config file not found: {}", path))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("src-tauri is nested under gui")
        .to_path_buf()
}

fn relative_to_repo(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn write_document_and_resolve(
    resolved_path: &Path,
    document: DocumentMut,
) -> Result<ResolvedPipelineGraph, String> {
    let next_content = document.to_string();
    let config = toml::from_str(&next_content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    fs::write(resolved_path, next_content)
        .map_err(|error| format!("Failed to write '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

fn update_existing_parameter(
    document: &mut DocumentMut,
    node_id: &str,
    parameter_key: &str,
    value: &str,
) -> Result<(), String> {
    let stage = stage_item_mut(document, node_id)?;
    let parameters = stage
        .get_mut("parameters")
        .ok_or_else(|| format!("Node '{}' has no parameters table", node_id))?;

    if let Some(table) = parameters.as_table_mut() {
        let existing = table
            .get_mut(parameter_key)
            .ok_or_else(|| format!("Parameter '{}' does not exist", parameter_key))?;
        set_existing_scalar(existing, value)
    } else if let Some(inline_table) = parameters.as_inline_table_mut() {
        let existing = inline_table
            .get_mut(parameter_key)
            .ok_or_else(|| format!("Parameter '{}' does not exist", parameter_key))?;
        set_existing_scalar_value(existing, value)
    } else {
        Err(format!(
            "Node '{}' parameters are not editable as a table",
            node_id
        ))
    }
}

fn update_json_parameter(
    document: &mut DocumentMut,
    node_id: &str,
    parameter_key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let stage = stage_item_mut(document, node_id)?;

    if stage.get("parameters").is_none() {
        stage["parameters"] = toml_edit::value(toml_edit::InlineTable::new());
    }

    let parameters = stage
        .get_mut("parameters")
        .ok_or_else(|| format!("Node '{}' has no editable parameters table", node_id))?;
    let next_item = toml_item_from_json(value)?;

    if let Some(table) = parameters.as_table_mut() {
        table[parameter_key] = next_item;
        Ok(())
    } else if let Some(inline_table) = parameters.as_inline_table_mut() {
        let next_value = next_item
            .into_value()
            .map_err(|_| "Inline parameters cannot store this nested value".to_string())?;
        inline_table.insert(parameter_key, next_value);
        Ok(())
    } else {
        Err(format!(
            "Node '{}' parameters are not editable as a table",
            node_id
        ))
    }
}

fn add_input_channel(
    document: &mut DocumentMut,
    target_node_id: &str,
    channel_name: &str,
) -> Result<(), String> {
    let stage = stage_item_mut(document, target_node_id)?;
    let inputs = inputs_array_mut(stage)?;

    if inputs
        .iter()
        .any(|input| input.as_str() == Some(channel_name))
    {
        return Err(format!("Channel '{}' is already connected", channel_name));
    }

    inputs.push(channel_name);
    Ok(())
}

fn remove_input_channel(
    document: &mut DocumentMut,
    target_node_id: &str,
    channel_name: &str,
) -> Result<(), String> {
    let stage = stage_item_mut(document, target_node_id)?;
    let inputs = inputs_array_mut(stage)?;
    let mut next_inputs = Array::default();
    let mut removed = false;

    for input in inputs.iter() {
        let Some(input_name) = input.as_str() else {
            return Err("Stage inputs must be string channel names".to_string());
        };

        if input_name == channel_name {
            removed = true;
        } else {
            next_inputs.push(input_name);
        }
    }

    if !removed {
        return Err(format!("Channel '{}' is not connected", channel_name));
    }

    *inputs = next_inputs;
    Ok(())
}

fn inputs_array_mut(stage: &mut Item) -> Result<&mut Array, String> {
    if stage.get("inputs").is_none() {
        stage["inputs"] = toml_edit::value(Array::default());
    }

    stage
        .get_mut("inputs")
        .and_then(Item::as_value_mut)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Stage inputs are not editable as an array".to_string())
}

fn toml_item_from_json(value: &serde_json::Value) -> Result<Item, String> {
    let snippet = format!("value = {}", toml_literal_from_json(value)?);
    let mut document = snippet
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to convert JSON parameter to TOML: {}", error))?;

    Ok(document.remove("value").unwrap_or(Item::None))
}

fn toml_literal_from_json(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Null => Err("TOML parameters cannot be null".to_string()),
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(Value::from(value.as_str()).to_string()),
        serde_json::Value::Array(values) => values
            .iter()
            .map(toml_literal_from_json)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| format!("[{}]", values.join(", "))),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok(format!("{} = {}", toml_key(key), toml_literal_from_json(value)?)))
            .collect::<Result<Vec<_>, String>>()
            .map(|fields| format!("{{ {} }}", fields.join(", "))),
    }
}

fn toml_key(key: &str) -> String {
    if key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        key.to_string()
    } else {
        Value::from(key).to_string()
    }
}

fn update_existing_field(
    document: &mut DocumentMut,
    node_id: &str,
    field_key: &str,
    value: &str,
) -> Result<(), String> {
    let stage = stage_item_mut(document, node_id)?;

    match field_key {
        "output" => {
            let output = stage
                .get_mut("output")
                .ok_or_else(|| format!("Node '{}' has no editable output field", node_id))?;
            set_existing_scalar(output, value)
        }
        "channel.type" => set_channel_field(stage, "type", value),
        "channel.capacity" => set_channel_field(stage, "capacity", value),
        "concurrency.type" => set_concurrency_field(stage, value),
        "timing.event_time_field" => set_timing_field(stage, "event_time_field", value),
        "timing.max_lateness_ms" => set_timing_field(stage, "max_lateness_ms", value),
        "timing.processing_timeout_ms" => set_timing_field(stage, "processing_timeout_ms", value),
        "timing.jitter_bounds_ms" => set_timing_field(stage, "jitter_bounds_ms", value),
        "timing.metrics_enabled" => set_timing_field(stage, "metrics_enabled", value),
        _ => Err(format!("Unsupported editable field: {}", field_key)),
    }
}

fn set_channel_field(stage: &mut Item, field_key: &str, value: &str) -> Result<(), String> {
    if stage.get("output").is_none() {
        return Err("Only output-producing stages have channel settings".to_string());
    }

    if stage.get("channel").is_none() {
        stage["channel"] = toml_edit::value(toml_edit::InlineTable::new());
    }

    let channel = stage
        .get_mut("channel")
        .ok_or_else(|| "Channel settings are not editable on this stage".to_string())?;

    if let Some(table) = channel.as_table_mut() {
        match field_key {
            "type" => {
                table["type"] = toml_edit::value(value);
                Ok(())
            }
            "capacity" => {
                let parsed = parse_non_negative_integer(value, "capacity")?;
                table["capacity"] = toml_edit::value(parsed);
                Ok(())
            }
            _ => Err(format!("Unsupported channel field: {}", field_key)),
        }
    } else if let Some(inline_table) = channel.as_inline_table_mut() {
        match field_key {
            "type" => {
                inline_table.insert("type", Value::from(value));
                Ok(())
            }
            "capacity" => {
                let parsed = parse_non_negative_integer(value, "capacity")?;
                inline_table.insert("capacity", Value::from(parsed));
                Ok(())
            }
            _ => Err(format!("Unsupported channel field: {}", field_key)),
        }
    } else {
        Err("Channel settings are not editable as a table".to_string())
    }
}

fn set_concurrency_field(stage: &mut Item, value: &str) -> Result<(), String> {
    ensure_inline_table(stage, "concurrency");
    let concurrency = stage
        .get_mut("concurrency")
        .ok_or_else(|| "Concurrency settings are not editable on this stage".to_string())?;

    set_nested_string(concurrency, "type", value)
}

fn set_timing_field(stage: &mut Item, field_key: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        && matches!(
            field_key,
            "event_time_field" | "processing_timeout_ms" | "jitter_bounds_ms"
        )
    {
        if let Some(timing) = stage.get_mut("timing") {
            remove_nested_field(timing, field_key);
        }
        return Ok(());
    }

    ensure_inline_table(stage, "timing");
    let timing = stage
        .get_mut("timing")
        .ok_or_else(|| "Timing settings are not editable on this stage".to_string())?;

    match field_key {
        "event_time_field" => set_nested_string(timing, field_key, value),
        "max_lateness_ms" | "processing_timeout_ms" | "jitter_bounds_ms" => {
            let parsed = parse_non_negative_integer(value, field_key)?;
            set_nested_integer(timing, field_key, parsed)
        }
        "metrics_enabled" => {
            let parsed = value
                .parse::<bool>()
                .map_err(|_| format!("'{}' is not a boolean", value))?;
            set_nested_bool(timing, field_key, parsed)
        }
        _ => Err(format!("Unsupported timing field: {}", field_key)),
    }
}

fn ensure_inline_table(stage: &mut Item, field_key: &str) {
    if stage.get(field_key).is_none() {
        stage[field_key] = toml_edit::value(toml_edit::InlineTable::new());
    }
}

fn set_nested_string(item: &mut Item, field_key: &str, value: &str) -> Result<(), String> {
    if let Some(table) = item.as_table_mut() {
        table[field_key] = toml_edit::value(value);
        return Ok(());
    }

    if let Some(inline_table) = item.as_inline_table_mut() {
        inline_table.insert(field_key, Value::from(value));
        return Ok(());
    }

    Err(format!("'{}' is not editable as a table field", field_key))
}

fn set_nested_integer(item: &mut Item, field_key: &str, value: i64) -> Result<(), String> {
    if let Some(table) = item.as_table_mut() {
        table[field_key] = toml_edit::value(value);
        return Ok(());
    }

    if let Some(inline_table) = item.as_inline_table_mut() {
        inline_table.insert(field_key, Value::from(value));
        return Ok(());
    }

    Err(format!("'{}' is not editable as a table field", field_key))
}

fn set_nested_bool(item: &mut Item, field_key: &str, value: bool) -> Result<(), String> {
    if let Some(table) = item.as_table_mut() {
        table[field_key] = toml_edit::value(value);
        return Ok(());
    }

    if let Some(inline_table) = item.as_inline_table_mut() {
        inline_table.insert(field_key, Value::from(value));
        return Ok(());
    }

    Err(format!("'{}' is not editable as a table field", field_key))
}

fn remove_nested_field(item: &mut Item, field_key: &str) {
    if let Some(table) = item.as_table_mut() {
        table.remove(field_key);
    } else if let Some(inline_table) = item.as_inline_table_mut() {
        inline_table.remove(field_key);
    }
}

fn parse_non_negative_integer(value: &str, label: &str) -> Result<i64, String> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| format!("'{}' is not an integer {}", value, label))?;

    if parsed < 0 {
        return Err(format!("'{}' cannot be negative", label));
    }

    Ok(parsed)
}

fn stage_item_mut<'a>(document: &'a mut DocumentMut, node_id: &str) -> Result<&'a mut Item, String> {
    if let Some(name) = node_id.strip_prefix("input:") {
        return document
            .get_mut("inputs")
            .and_then(|item| item.get_mut(name))
            .ok_or_else(|| format!("Input node '{}' not found", name));
    }

    if let Some(name) = node_id.strip_prefix("output:") {
        return document
            .get_mut("outputs")
            .and_then(|item| item.get_mut(name))
            .ok_or_else(|| format!("Output node '{}' not found", name));
    }

    if let Some(rest) = node_id.strip_prefix("pipeline:") {
        let (pipeline_name, stage_name) = rest
            .split_once(".stage:")
            .ok_or_else(|| format!("Invalid pipeline node id: {}", node_id))?;

        return document
            .get_mut("pipelines")
            .and_then(|item| item.get_mut(pipeline_name))
            .and_then(|item| item.get_mut("stages"))
            .and_then(|item| item.get_mut(stage_name))
            .ok_or_else(|| {
                format!(
                    "Pipeline stage '{}.{}' not found",
                    pipeline_name, stage_name
                )
            });
    }

    Err(format!("Unsupported node id: {}", node_id))
}

fn set_existing_scalar(item: &mut Item, value: &str) -> Result<(), String> {
    let existing = item
        .as_value_mut()
        .ok_or_else(|| "Nested parameters are read-only in this phase".to_string())?;
    set_existing_scalar_value(existing, value)
}

fn set_existing_scalar_value(existing: &mut Value, value: &str) -> Result<(), String> {
    if existing.is_str() {
        *existing = Value::from(value);
        return Ok(());
    }

    if existing.is_bool() {
        let parsed = value
            .parse::<bool>()
            .map_err(|_| format!("'{}' is not a boolean", value))?;
        *existing = Value::from(parsed);
        return Ok(());
    }

    if existing.is_integer() {
        let parsed = value
            .parse::<i64>()
            .map_err(|_| format!("'{}' is not an integer", value))?;
        *existing = Value::from(parsed);
        return Ok(());
    }

    if existing.is_float() {
        let parsed = value
            .parse::<f64>()
            .map_err(|_| format!("'{}' is not a number", value))?;
        *existing = Value::from(parsed);
        return Ok(());
    }

    Err("Nested parameters are read-only in this phase".to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_graph,
            connect_nodes,
            disconnect_edge,
            list_example_configs,
            list_processor_descriptors,
            update_node_field,
            update_node_parameter_json,
            update_node_parameter
        ])
        .run(tauri::generate_context!())
        .expect("error while running Liminal Pipeline GUI");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn updates_inline_parameter_and_preserves_surrounding_comments() {
        let mut document = parse_document(
            r#"
# sensor source
[inputs.sensor]
type = "simulated"
output = "raw_data"
parameters = { field_out = "value", interval_ms = 1000 } # keep this comment

[outputs.console]
type = "console"
inputs = ["raw_data"]
"#,
        );

        update_existing_parameter(&mut document, "input:sensor", "interval_ms", "2500")
            .expect("inline parameter update succeeds");

        let edited = document.to_string();
        assert!(edited.contains("# sensor source"));
        assert!(edited.contains("# keep this comment"));
        assert!(edited.contains("field_out = \"value\""));
        assert!(edited.contains("interval_ms = 2500"));
    }

    #[test]
    fn updates_table_parameter_without_rewriting_nested_rules() {
        let mut document = parse_document(
            r#"
[pipelines.rules]
description = "Rule pipeline"

[pipelines.rules.stages.filter]
type = "rule"
inputs = ["raw_data"]
output = "filtered_data"

[pipelines.rules.stages.filter.parameters]
error_strategy = "log_and_continue"

[[pipelines.rules.stages.filter.parameters.rules]]
# nested rule stays readable
condition = { field_path = "device_id", operation = "startswith", value = "esp32" }
actions = [{ type = "pass_through" }]
"#,
        );

        update_existing_parameter(
            &mut document,
            "pipeline:rules.stage:filter",
            "error_strategy",
            "drop",
        )
        .expect("table parameter update succeeds");

        let edited = document.to_string();
        assert!(edited.contains("error_strategy = \"drop\""));
        assert!(edited.contains("# nested rule stays readable"));
        assert!(edited.contains("[[pipelines.rules.stages.filter.parameters.rules]]"));
        assert!(edited.contains("actions = [{ type = \"pass_through\" }]"));
    }

    #[test]
    fn updates_inline_channel_type_and_capacity() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
channel = { type = "broadcast", capacity = 128 }
"#,
        );

        update_existing_field(&mut document, "input:sensor", "channel.type", "fanout")
            .expect("channel type update succeeds");
        update_existing_field(&mut document, "input:sensor", "channel.capacity", "512")
            .expect("channel capacity update succeeds");

        let edited = document.to_string();
        assert!(edited.contains("type = \"fanout\""));
        assert!(edited.contains("capacity = 512"));
    }

    #[test]
    fn updates_table_channel_type_and_capacity() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[inputs.sensor.channel]
type = "broadcast"
capacity = 128
"#,
        );

        update_existing_field(&mut document, "input:sensor", "channel.type", "direct")
            .expect("channel table type update succeeds");
        update_existing_field(&mut document, "input:sensor", "channel.capacity", "32")
            .expect("channel table capacity update succeeds");

        let edited = document.to_string();
        assert!(edited.contains("[inputs.sensor.channel]"));
        assert!(edited.contains("type = \"direct\""));
        assert!(edited.contains("capacity = 32"));
    }

    #[test]
    fn creates_inline_channel_table_for_output_producing_stage() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
"#,
        );

        update_existing_field(&mut document, "input:sensor", "channel.capacity", "256")
            .expect("missing channel table is created");

        let edited = document.to_string();
        assert!(edited.contains("channel = {"));
        assert!(edited.contains("capacity = 256"));
    }

    #[test]
    fn creates_and_updates_concurrency_table() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
"#,
        );

        update_existing_field(&mut document, "input:sensor", "concurrency.type", "pipeline")
            .expect("concurrency type update succeeds");

        let edited = document.to_string();
        assert!(edited.contains("concurrency = {"));
        assert!(edited.contains("type = \"pipeline\""));
    }

    #[test]
    fn updates_inline_timing_fields() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
timing = { event_time_field = "ts", max_lateness_ms = 30000, metrics_enabled = true }
"#,
        );

        update_existing_field(
            &mut document,
            "input:sensor",
            "timing.event_time_field",
            "event_ts",
        )
        .expect("timing event field update succeeds");
        update_existing_field(
            &mut document,
            "input:sensor",
            "timing.max_lateness_ms",
            "15000",
        )
        .expect("timing lateness update succeeds");
        update_existing_field(
            &mut document,
            "input:sensor",
            "timing.metrics_enabled",
            "false",
        )
        .expect("timing boolean update succeeds");

        let edited = document.to_string();
        assert!(edited.contains("event_time_field = \"event_ts\""));
        assert!(edited.contains("max_lateness_ms = 15000"));
        assert!(edited.contains("metrics_enabled = false"));
    }

    #[test]
    fn updates_table_timing_fields_and_clears_optional_values() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[inputs.sensor.timing]
event_time_field = "ts"
processing_timeout_ms = 1000
jitter_bounds_ms = 50
"#,
        );

        update_existing_field(
            &mut document,
            "input:sensor",
            "timing.processing_timeout_ms",
            "2000",
        )
        .expect("timing timeout update succeeds");
        update_existing_field(&mut document, "input:sensor", "timing.event_time_field", "")
            .expect("blank optional timing string clears the field");
        update_existing_field(&mut document, "input:sensor", "timing.jitter_bounds_ms", "")
            .expect("blank optional timing number clears the field");

        let edited = document.to_string();
        assert!(edited.contains("[inputs.sensor.timing]"));
        assert!(edited.contains("processing_timeout_ms = 2000"));
        assert!(!edited.contains("event_time_field"));
        assert!(!edited.contains("jitter_bounds_ms"));
    }

    #[test]
    fn rejects_invalid_timing_values() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
"#,
        );

        let number_error = update_existing_field(
            &mut document,
            "input:sensor",
            "timing.max_lateness_ms",
            "-10",
        )
        .expect_err("negative timing value is rejected");
        let bool_error = update_existing_field(
            &mut document,
            "input:sensor",
            "timing.metrics_enabled",
            "maybe",
        )
        .expect_err("invalid timing bool is rejected");

        assert!(number_error.contains("cannot be negative"));
        assert!(bool_error.contains("not a boolean"));
    }

    #[test]
    fn rejects_nested_parameter_edit() {
        let mut document = parse_document(
            r#"
[pipelines.rules]
description = "Rule pipeline"

[pipelines.rules.stages.filter]
type = "rule"
inputs = ["raw_data"]
output = "filtered_data"

[[pipelines.rules.stages.filter.parameters.rules]]
condition = { field_path = "device_id", operation = "exists" }
actions = [{ type = "pass_through" }]
"#,
        );

        let error =
            update_existing_parameter(&mut document, "pipeline:rules.stage:filter", "rules", "[]")
                .expect_err("nested parameter edits are rejected");

        assert!(error.contains("Nested parameters are read-only"));
    }

    #[test]
    fn updates_nested_rules_parameter_from_json() {
        let mut document = parse_document(
            r#"
[pipelines.rules]
description = "Rule pipeline"

[pipelines.rules.stages.filter]
type = "rule"
inputs = ["raw_data"]
output = "filtered_data"

[[pipelines.rules.stages.filter.parameters.rules]]
condition = { field_path = "device_id", operation = "startswith", value = "esp32" }
actions = [{ type = "pass_through" }]
"#,
        );
        let value = serde_json::json!([
            {
                "condition": { "field_path": "temperature", "operation": ">", "value": 15 },
                "actions": [
                    { "type": "set_field", "field_path": "state", "value": true }
                ],
                "else_actions": [
                    { "type": "set_field", "field_path": "state", "value": false }
                ]
            }
        ]);

        update_json_parameter(
            &mut document,
            "pipeline:rules.stage:filter",
            "rules",
            &value,
        )
        .expect("nested rule update succeeds");

        let edited = document.to_string();
        let parsed: serde_json::Value = toml::from_str::<toml::Value>(&edited)
            .expect("edited TOML parses")
            .try_into()
            .expect("edited TOML converts to JSON");

        assert!(edited.contains("temperature"));
        assert!(edited.contains("set_field"));
        assert_eq!(
            parsed["pipelines"]["rules"]["stages"]["filter"]["parameters"]["rules"][0]
                ["condition"]["field_path"],
            "temperature"
        );
    }

    #[test]
    fn adds_input_channel_when_connecting_nodes() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[outputs.console]
type = "console"
"#,
        );

        add_input_channel(&mut document, "output:console", "raw_data")
            .expect("input channel is added");

        let edited = document.to_string();
        assert!(edited.contains("inputs = [\"raw_data\"]"));
    }

    #[test]
    fn rejects_duplicate_input_channel_connection() {
        let mut document = parse_document(
            r#"
[outputs.console]
type = "console"
inputs = ["raw_data"]
"#,
        );

        let error = add_input_channel(&mut document, "output:console", "raw_data")
            .expect_err("duplicate input channel is rejected");

        assert!(error.contains("already connected"));
    }

    #[test]
    fn removes_input_channel_when_disconnects_edge() {
        let mut document = parse_document(
            r#"
[outputs.console]
type = "console"
inputs = ["raw_data", "filtered_data"]
"#,
        );

        remove_input_channel(&mut document, "output:console", "raw_data")
            .expect("input channel is removed");

        let edited = document.to_string();
        assert!(!edited.contains("\"raw_data\""));
        assert!(edited.contains("\"filtered_data\""));
    }

    #[test]
    fn rejects_invalid_scalar_type() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
parameters = { interval_ms = 1000, enabled = true }
"#,
        );

        let number_error = update_existing_parameter(
            &mut document,
            "input:sensor",
            "interval_ms",
            "not-a-number",
        )
        .expect_err("invalid integer is rejected");
        let bool_error =
            update_existing_parameter(&mut document, "input:sensor", "enabled", "sometimes")
                .expect_err("invalid boolean is rejected");

        assert!(number_error.contains("not an integer"));
        assert!(bool_error.contains("not a boolean"));
    }

    #[test]
    fn rejects_invalid_config_without_overwriting_file() {
        let content = r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
channel = { type = "broadcast", capacity = 128 }

[outputs.console]
type = "console"
inputs = ["raw_data"]
"#;
        let path = unique_temp_config_path();
        fs::write(&path, content).expect("test config is written");

        let result = update_node_field(
            path.to_string_lossy().to_string(),
            "input:sensor".to_string(),
            "channel.capacity".to_string(),
            "-1".to_string(),
        );

        let persisted = fs::read_to_string(&path).expect("test config can be reread");
        let _ = fs::remove_file(&path);

        assert!(result.is_err());
        assert_eq!(persisted, content);
    }

    fn parse_document(content: &str) -> DocumentMut {
        content.parse::<DocumentMut>().expect("test TOML parses")
    }

    fn unique_temp_config_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after UNIX epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "liminal-gui-edit-test-{}-{}.toml",
            std::process::id(),
            nanos
        ))
    }
}
