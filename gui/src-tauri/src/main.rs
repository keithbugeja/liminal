use liminal::config::{load_config, Config, ResolvedPipelineGraph};
use liminal::processors::create_processor;
use liminal::processors::descriptor::{
    processor_descriptors, FieldKind, FieldSpec, ProcessorCategory, ProcessorDescriptor,
};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{Emitter, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

#[derive(Clone, Default)]
struct PipelineRuntime {
    process_id: Arc<StdMutex<Option<u32>>>,
}

#[derive(Serialize)]
struct DraftEditResult {
    graph: ResolvedPipelineGraph,
    content: String,
}

#[derive(Clone, Serialize)]
struct PipelineLogEvent {
    stream: String,
    line: String,
}

#[derive(Clone, Serialize)]
struct PipelineStateEvent {
    state: String,
    message: Option<String>,
}

#[tauri::command]
fn load_graph(path: String) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let config = load_config(&resolved_path)
        .map_err(|error| format!("Failed to load '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn load_config_text(path: String) -> Result<String, String> {
    let resolved_path = resolve_config_path(&path)?;
    fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))
}

#[tauri::command]
fn save_config_text(path: String, content: String) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let config = toml::from_str::<Config>(&content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    fs::write(&resolved_path, content)
        .map_err(|error| format!("Failed to write '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn save_config_as(path: String, content: String) -> Result<ResolvedPipelineGraph, String> {
    let target_path = writable_config_path(&path)?;
    let config = toml::from_str::<Config>(&content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create '{}': {}", parent.display(), error))?;
    }

    fs::write(&target_path, content)
        .map_err(|error| format!("Failed to write '{}': {}", target_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
fn copy_config_to_workspace(
    workspace_path: String,
    source_path: String,
    content: String,
) -> Result<String, String> {
    let target_path = copy_config_to_workspace_path(&workspace_path, &source_path, &content)?;
    Ok(relative_to_repo(&target_path))
}

#[tauri::command]
fn start_pipeline(
    window: tauri::Window,
    runtime: State<'_, PipelineRuntime>,
    path: String,
) -> Result<(), String> {
    let resolved_path = resolve_config_path(&path)?;
    let config = load_config(&resolved_path)
        .map_err(|error| format!("Failed to load '{}': {}", resolved_path.display(), error))?;
    liminal::config::validate_config(&config)
        .map_err(|error| format!("Configuration error: {}", error))?;
    preflight_processors(&config)?;

    {
        let running_process = runtime
            .process_id
            .lock()
            .map_err(|_| "Pipeline runtime lock is poisoned".to_string())?;
        if running_process.is_some() {
            return Err("A pipeline is already running.".to_string());
        }
    }

    let mut command = pipeline_command(&resolved_path);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start pipeline: {}", error))?;

    let process_id = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut running_process = runtime
            .process_id
            .lock()
            .map_err(|_| "Pipeline runtime lock is poisoned".to_string())?;
        *running_process = Some(process_id);
    }

    emit_pipeline_state(&window, "running", Some(format!("Started process {}", process_id)));

    if let Some(stdout) = stdout {
        spawn_pipeline_log_reader(window.clone(), "stdout", stdout);
    }

    if let Some(stderr) = stderr {
        spawn_pipeline_log_reader(window.clone(), "stderr", stderr);
    }

    let runtime_process_id = runtime.process_id.clone();
    std::thread::spawn(move || {
        let exit_result = child.wait();
        if let Ok(mut running_process) = runtime_process_id.lock() {
            if *running_process == Some(process_id) {
                *running_process = None;
            }
        }

        let message = match exit_result {
            Ok(status) => Some(format!("Pipeline exited with status {}", status)),
            Err(error) => Some(format!("Failed while waiting for pipeline: {}", error)),
        };
        emit_pipeline_state(&window, "stopped", message);
    });

    Ok(())
}

fn preflight_processors(config: &Config) -> Result<(), String> {
    for (name, stage) in &config.inputs {
        create_processor(&stage.r#type, stage.clone()).map_err(|error| {
            format!(
                "Runtime preflight failed for input '{}': processor '{}' could not be created: {}",
                name, stage.r#type, error
            )
        })?;
    }

    for (pipeline_name, pipeline) in &config.pipelines {
        for (stage_name, stage) in &pipeline.stages {
            create_processor(&stage.r#type, stage.clone()).map_err(|error| {
                format!(
                    "Runtime preflight failed for stage '{}.{}': processor '{}' could not be created: {}",
                    pipeline_name, stage_name, stage.r#type, error
                )
            })?;
        }
    }

    for (name, stage) in &config.outputs {
        create_processor(&stage.r#type, stage.clone()).map_err(|error| {
            format!(
                "Runtime preflight failed for output '{}': processor '{}' could not be created: {}",
                name, stage.r#type, error
            )
        })?;
    }

    Ok(())
}

#[tauri::command]
fn stop_pipeline(runtime: State<'_, PipelineRuntime>) -> Result<(), String> {
    let process_id = {
        let mut running_process = runtime
            .process_id
            .lock()
            .map_err(|_| "Pipeline runtime lock is poisoned".to_string())?;
        running_process.take()
    };

    let Some(process_id) = process_id else {
        return Ok(());
    };

    terminate_process(process_id)
}

#[tauri::command]
fn pipeline_runtime_state(runtime: State<'_, PipelineRuntime>) -> Result<String, String> {
    let running_process = runtime
        .process_id
        .lock()
        .map_err(|_| "Pipeline runtime lock is poisoned".to_string())?;
    Ok(if running_process.is_some() {
        "running".to_string()
    } else {
        "idle".to_string()
    })
}

#[tauri::command]
fn list_workspace_configs(path: String) -> Result<Vec<String>, String> {
    let folder = PathBuf::from(&path);
    let folder = if folder.is_absolute() {
        folder
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(folder)
    };

    if !folder.is_dir() {
        return Err(format!("Workspace folder not found: {}", path));
    }

    let mut configs = Vec::new();
    collect_toml_files(&folder, &mut configs)?;
    configs.sort();
    Ok(configs)
}

#[tauri::command]
async fn pick_config_file(window: tauri::Window) -> Result<Option<String>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();

    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Open Config")
        .add_filter("TOML config", &["toml"])
        .pick_file(move |selected| {
            let _ = sender.send(selected.map(dialog_path_to_string).transpose());
        });

    wait_for_dialog_path(receiver).await
}

#[tauri::command]
async fn pick_workspace_folder(window: tauri::Window) -> Result<Option<String>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();

    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Open Workspace Folder")
        .pick_folder(move |selected| {
            let _ = sender.send(selected.map(dialog_path_to_string).transpose());
        });

    wait_for_dialog_path(receiver).await
}

#[tauri::command]
async fn pick_save_config_path(
    window: tauri::Window,
    default_path: String,
) -> Result<Option<String>, String> {
    let mut dialog = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Save Config As")
        .add_filter("TOML config", &["toml"]);
    let default_path = PathBuf::from(default_path);

    if let Some(parent) = default_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        dialog = dialog.set_directory(parent);
    }

    if let Some(file_name) = default_path.file_name() {
        dialog = dialog.set_file_name(file_name.to_string_lossy().to_string());
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    dialog.save_file(move |selected| {
        let _ = sender.send(selected.map(dialog_path_to_string).transpose());
    });

    wait_for_dialog_path(receiver).await
}

#[tauri::command]
fn update_node_parameter_draft(
    content: String,
    node_id: String,
    parameter_key: String,
    value: String,
) -> Result<DraftEditResult, String> {
    let mut document = document_from_content(&content)?;
    update_existing_parameter(&mut document, &node_id, &parameter_key, &value)?;
    draft_result_from_document(document)
}

#[tauri::command]
fn update_node_parameter_json_draft(
    content: String,
    node_id: String,
    parameter_key: String,
    value_json: String,
) -> Result<DraftEditResult, String> {
    let value = serde_json::from_str::<serde_json::Value>(&value_json)
        .map_err(|error| format!("Failed to parse parameter JSON: {}", error))?;
    let mut document = document_from_content(&content)?;
    update_json_parameter(&mut document, &node_id, &parameter_key, &value)?;
    draft_result_from_document(document)
}

#[tauri::command]
fn update_node_field_draft(
    content: String,
    node_id: String,
    field_key: String,
    value: String,
) -> Result<DraftEditResult, String> {
    let mut document = document_from_content(&content)?;
    update_existing_field(&mut document, &node_id, &field_key, &value)?;
    draft_result_from_document(document)
}

#[tauri::command]
fn connect_nodes_draft(
    content: String,
    source_node_id: String,
    target_node_id: String,
) -> Result<DraftEditResult, String> {
    let channel_name = channel_for_connection(&content, &source_node_id, &target_node_id)?;
    let mut document = document_from_content(&content)?;
    add_input_channel(&mut document, &target_node_id, &channel_name)?;
    draft_result_from_document(document)
}

#[tauri::command]
fn disconnect_edge_draft(
    content: String,
    target_node_id: String,
    channel_name: String,
) -> Result<DraftEditResult, String> {
    if target_node_id.starts_with("input:") {
        return Err("Input stages do not have input channel lists".to_string());
    }

    let mut document = document_from_content(&content)?;
    remove_input_channel(&mut document, &target_node_id, &channel_name)?;
    draft_result_from_document(document)
}

#[tauri::command]
fn add_node_draft(
    content: String,
    processor_type: String,
    node_name: String,
    pipeline_name: Option<String>,
) -> Result<DraftEditResult, String> {
    let descriptor = descriptor_for_type(&processor_type)?;
    let node_name = node_name.trim();
    validate_node_name(node_name)?;

    let mut document = document_from_content(&content)?;
    add_node_to_document(
        &mut document,
        &descriptor,
        node_name,
        pipeline_name.as_deref(),
    )?;
    draft_result_from_document(document)
}

#[tauri::command]
fn delete_node_draft(content: String, node_id: String) -> Result<DraftEditResult, String> {
    let graph = graph_from_content(&content, "deleting node")?;
    let output_channel = graph
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| format!("Node '{}' not found", node_id))?
        .output_channel
        .clone();
    let mut document = document_from_content(&content)?;

    delete_node_from_document(&mut document, &node_id)?;
    if let Some(channel_name) = output_channel {
        remove_channel_from_all_inputs(&mut document, &channel_name)?;
    }

    draft_result_from_document(document)
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
    let resolved_path = resolve_config_path(&path)?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let channel_name = channel_for_connection(&content, &source_node_id, &target_node_id)?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    add_input_channel(&mut document, &target_node_id, &channel_name)?;
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
fn delete_node(path: String, node_id: String) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let config = toml::from_str::<Config>(&content)
        .map_err(|error| format!("Failed to parse config before deleting node: {}", error))?;
    let graph = ResolvedPipelineGraph::from_config(&config);
    let output_channel = graph
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| format!("Node '{}' not found", node_id))?
        .output_channel
        .clone();
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    delete_node_from_document(&mut document, &node_id)?;
    if let Some(channel_name) = output_channel {
        remove_channel_from_all_inputs(&mut document, &channel_name)?;
    }

    write_document_and_resolve(&resolved_path, document)
}

#[tauri::command]
fn add_node(
    path: String,
    processor_type: String,
    node_name: String,
    pipeline_name: Option<String>,
) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let descriptor = descriptor_for_type(&processor_type)?;
    let node_name = node_name.trim();

    validate_node_name(node_name)?;

    let content = fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))?;

    add_node_to_document(
        &mut document,
        &descriptor,
        node_name,
        pipeline_name.as_deref(),
    )?;
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
    let mut configs = vec![relative_to_repo(&repo_root().join("config").join("config.toml"))];

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

fn writable_config_path(path: &str) -> Result<PathBuf, String> {
    let trimmed_path = path.trim();
    if trimmed_path.is_empty() {
        return Err("Save path cannot be empty".to_string());
    }

    let path = PathBuf::from(trimmed_path);
    let resolved_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };

    if resolved_path
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("toml")
    {
        return Err("Config files must use the .toml extension".to_string());
    }

    Ok(resolved_path)
}

fn collect_toml_files(folder: &Path, configs: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(folder)
        .map_err(|error| format!("Failed to read '{}': {}", folder.display(), error))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            collect_toml_files(&path, configs)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
            configs.push(relative_to_repo(&path));
        }
    }

    Ok(())
}

fn copy_config_to_workspace_path(
    workspace_path: &str,
    source_path: &str,
    content: &str,
) -> Result<PathBuf, String> {
    toml::from_str::<Config>(content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    let workspace = writable_directory_path(workspace_path)?;
    let source = PathBuf::from(source_path);
    let file_name = source
        .file_name()
        .ok_or_else(|| "Source config path does not include a file name".to_string())?;
    let target_path = writable_config_path(&workspace.join(file_name).to_string_lossy())?;

    fs::create_dir_all(&workspace)
        .map_err(|error| format!("Failed to create '{}': {}", workspace.display(), error))?;

    if target_path.exists() {
        let source_resolved = resolve_config_path(source_path).ok();
        let same_file = source_resolved
            .and_then(|path| path.canonicalize().ok())
            .zip(target_path.canonicalize().ok())
            .is_some_and(|(source, target)| source == target);

        if !same_file {
            return Err(format!(
                "Workspace already contains '{}'. Use Save As to choose an explicit destination.",
                target_path.display()
            ));
        }
    }

    fs::write(&target_path, content)
        .map_err(|error| format!("Failed to write '{}': {}", target_path.display(), error))?;

    Ok(target_path)
}

fn writable_directory_path(path: &str) -> Result<PathBuf, String> {
    let trimmed_path = path.trim();
    if trimmed_path.is_empty() {
        return Err("Workspace path cannot be empty".to_string());
    }

    let path = PathBuf::from(trimmed_path);
    let resolved_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };

    Ok(resolved_path)
}

fn pipeline_command(config_path: &Path) -> Command {
    let mut command = if let Ok(binary_path) = std::env::var("LIMINAL_BIN") {
        Command::new(binary_path)
    } else if should_launch_runtime_with_cargo() {
        cargo_runtime_command()
    } else {
        let candidate = repo_root()
            .join("target")
            .join("debug")
            .join(format!("liminal{}", std::env::consts::EXE_SUFFIX));

        if candidate.exists() {
            Command::new(candidate)
        } else {
            let mut cargo = Command::new("cargo");
            cargo
                .arg("run")
                .arg("--quiet")
                .arg("--manifest-path")
                .arg(repo_root().join("Cargo.toml"))
                .arg("--");
            cargo
        }
    };

    command.arg("--config").arg(config_path);
    command.current_dir(repo_root());
    command
}

fn should_launch_runtime_with_cargo() -> bool {
    repo_root().join("Cargo.toml").is_file()
        && Command::new("cargo")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

fn cargo_runtime_command() -> Command {
    let mut cargo = Command::new("cargo");
    cargo
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .arg("--target-dir")
        .arg(repo_root().join("target").join("gui-runtime"))
        .arg("--");
    cargo
}

fn spawn_pipeline_log_reader<R>(window: tauri::Window, stream: &str, reader: R)
where
    R: std::io::Read + Send + 'static,
{
    let stream = stream.to_string();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else {
                break;
            };
            let _ = window.emit(
                "pipeline://log",
                PipelineLogEvent {
                    stream: stream.clone(),
                    line,
                },
            );
        }
    });
}

fn emit_pipeline_state(window: &tauri::Window, state: &str, message: Option<String>) {
    let _ = window.emit(
        "pipeline://state",
        PipelineStateEvent {
            state: state.to_string(),
            message,
        },
    );
}

fn terminate_process(process_id: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .arg("/PID")
            .arg(process_id.to_string())
            .arg("/T")
            .arg("/F")
            .status()
            .map_err(|error| format!("Failed to stop pipeline: {}", error))?;

        if !status.success() {
            return Err(format!("Failed to stop pipeline process {}", process_id));
        }
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(process_id.to_string())
            .status()
            .map_err(|error| format!("Failed to stop pipeline: {}", error))?;

        if !status.success() {
            return Err(format!("Failed to stop pipeline process {}", process_id));
        }
    }

    Ok(())
}

fn dialog_path_to_string(path: FilePath) -> Result<String, String> {
    path.simplified()
        .into_path()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| format!("Selected path is not a filesystem path: {}", error))
}

async fn wait_for_dialog_path(
    receiver: std::sync::mpsc::Receiver<Result<Option<String>, String>>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv()
            .unwrap_or_else(|_| Err("Dialog closed without returning a result".to_string()))
    })
    .await
    .map_err(|error| format!("Failed to receive dialog result: {}", error))?
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

fn document_from_content(content: &str) -> Result<DocumentMut, String> {
    content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse TOML for editing: {}", error))
}

fn draft_result_from_document(document: DocumentMut) -> Result<DraftEditResult, String> {
    let content = document.to_string();
    let graph = graph_from_content(&content, "editing draft")?;

    Ok(DraftEditResult { graph, content })
}

fn graph_from_content(content: &str, action: &str) -> Result<ResolvedPipelineGraph, String> {
    let config = toml::from_str::<Config>(content)
        .map_err(|error| format!("Failed to parse config before {}: {}", action, error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

fn descriptor_for_type(processor_type: &str) -> Result<ProcessorDescriptor, String> {
    processor_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.type_name == processor_type)
        .ok_or_else(|| format!("Unknown processor type '{}'", processor_type))
}

fn channel_for_connection(
    content: &str,
    source_node_id: &str,
    target_node_id: &str,
) -> Result<String, String> {
    if source_node_id == target_node_id {
        return Err("A node cannot be connected to itself".to_string());
    }

    if target_node_id.starts_with("input:") {
        return Err("Input stages cannot consume channels".to_string());
    }

    let graph = graph_from_content(content, "rewiring")?;
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

    if target_node
        .input_channels
        .iter()
        .any(|input| input == channel_name)
    {
        return Err(format!(
            "Target node '{}' already consumes channel '{}'",
            target_node.display_name, channel_name
        ));
    }

    Ok(channel_name.clone())
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
    } else if parameters.is_inline_table() {
        if !json_value_needs_table(value) {
            let next_value = next_item
                .clone()
                .into_value()
                .map_err(|_| "Inline parameters cannot store this nested value".to_string())?;
            let inline_table = parameters
                .as_inline_table_mut()
                .ok_or_else(|| format!("Node '{}' parameters are not editable as a table", node_id))?;
            inline_table.insert(parameter_key, next_value);
            Ok(())
        } else {
            let existing_inline = parameters
                .as_inline_table()
                .ok_or_else(|| format!("Node '{}' parameters are not editable as a table", node_id))?
                .clone();
            let mut table = Table::new();
            for (key, value) in existing_inline.iter() {
                table[key] = Item::Value(value.clone());
            }
            table[parameter_key] = next_item;
            *parameters = Item::Table(table);
            Ok(())
        }
    } else {
        Err(format!(
            "Node '{}' parameters are not editable as a table",
            node_id
        ))
    }
}

fn json_value_needs_table(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| matches!(value, serde_json::Value::Array(_) | serde_json::Value::Object(_))),
        serde_json::Value::Object(_) => true,
        _ => false,
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

fn add_node_to_document(
    document: &mut DocumentMut,
    descriptor: &ProcessorDescriptor,
    node_name: &str,
    pipeline_name: Option<&str>,
) -> Result<(), String> {
    let stage = new_node_table(descriptor, node_name)?;

    match descriptor.category {
        ProcessorCategory::Input => {
            let inputs = ensure_document_table(document, "inputs")?;
            insert_named_node(inputs, node_name, stage, "input")
        }
        ProcessorCategory::Output => {
            let outputs = ensure_document_table(document, "outputs")?;
            insert_named_node(outputs, node_name, stage, "output")
        }
        ProcessorCategory::Transform | ProcessorCategory::Aggregator => {
            let pipeline_name = pipeline_name
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or("default_pipeline");
            validate_node_name(pipeline_name)?;

            let pipelines = ensure_document_table(document, "pipelines")?;
            let pipeline = ensure_child_table(pipelines, pipeline_name)?;
            if pipeline.get("description").is_none() {
                pipeline["description"] = toml_edit::value(format!("{} pipeline", pipeline_name));
            }
            let stages = ensure_child_table(pipeline, "stages")?;
            insert_named_node(stages, node_name, stage, "pipeline stage")
        }
    }
}

fn delete_node_from_document(document: &mut DocumentMut, node_id: &str) -> Result<(), String> {
    if let Some(name) = node_id.strip_prefix("input:") {
        return remove_named_node(document, "inputs", name, "input");
    }

    if let Some(name) = node_id.strip_prefix("output:") {
        return remove_named_node(document, "outputs", name, "output");
    }

    if let Some(rest) = node_id.strip_prefix("pipeline:") {
        let (pipeline_name, stage_name) = rest
            .split_once(".stage:")
            .ok_or_else(|| format!("Invalid pipeline node id: {}", node_id))?;
        let stages = document
            .get_mut("pipelines")
            .and_then(|item| item.get_mut(pipeline_name))
            .and_then(|item| item.get_mut("stages"))
            .ok_or_else(|| {
                format!(
                    "Pipeline stage '{}.{}' not found",
                    pipeline_name, stage_name
                )
            })?;

        return remove_child_from_item(stages, stage_name, "pipeline stage");
    }

    Err(format!("Unsupported node id: {}", node_id))
}

fn remove_named_node(
    document: &mut DocumentMut,
    table_key: &str,
    node_name: &str,
    label: &str,
) -> Result<(), String> {
    let parent = document
        .get_mut(table_key)
        .ok_or_else(|| format!("{} table does not exist", table_key))?;

    remove_child_from_item(parent, node_name, label)
}

fn remove_child_from_item(parent: &mut Item, node_name: &str, label: &str) -> Result<(), String> {
    if let Some(table) = parent.as_table_mut() {
        table
            .remove(node_name)
            .map(|_| ())
            .ok_or_else(|| format!("A {} named '{}' does not exist", label, node_name))
    } else {
        Err(format!("{} parent is not editable as a table", label))
    }
}

fn remove_channel_from_all_inputs(
    document: &mut DocumentMut,
    channel_name: &str,
) -> Result<(), String> {
    if let Some(outputs) = document.get_mut("outputs") {
        remove_channel_from_stage_table(outputs, channel_name)?;
    }

    if let Some(pipelines) = document.get_mut("pipelines") {
        let Some(pipelines_table) = pipelines.as_table_mut() else {
            return Err("Pipelines are not editable as a table".to_string());
        };

        for (_, pipeline) in pipelines_table.iter_mut() {
            if let Some(stages) = pipeline.get_mut("stages") {
                remove_channel_from_stage_table(stages, channel_name)?;
            }
        }
    }

    Ok(())
}

fn remove_channel_from_stage_table(stages: &mut Item, channel_name: &str) -> Result<(), String> {
    let Some(stages_table) = stages.as_table_mut() else {
        return Err("Stages are not editable as a table".to_string());
    };

    for (_, stage) in stages_table.iter_mut() {
        remove_input_channel_if_present(stage, channel_name)?;
    }

    Ok(())
}

fn remove_input_channel_if_present(stage: &mut Item, channel_name: &str) -> Result<(), String> {
    let Some(inputs) = stage.get_mut("inputs") else {
        return Ok(());
    };
    let inputs = inputs
        .as_value_mut()
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Stage inputs are not editable as an array".to_string())?;
    let mut next_inputs = Array::default();

    for input in inputs.iter() {
        let Some(input_name) = input.as_str() else {
            return Err("Stage inputs must be string channel names".to_string());
        };

        if input_name != channel_name {
            next_inputs.push(input_name);
        }
    }

    *inputs = next_inputs;
    Ok(())
}

fn new_node_table(descriptor: &ProcessorDescriptor, node_name: &str) -> Result<Item, String> {
    let mut table = Table::new();
    table["type"] = toml_edit::value(descriptor.type_name.as_str());

    if descriptor.category != ProcessorCategory::Input {
        table["inputs"] = toml_edit::value(Array::default());
    }

    if descriptor.category != ProcessorCategory::Output {
        table["output"] = toml_edit::value(default_channel_name(node_name));
    }

    let mut parameters = toml_edit::InlineTable::new();
    for field in &descriptor.fields {
        if let Some(default_value) = default_value_for_field(field)? {
            parameters.insert(&field.key, default_value);
        }
    }

    if !parameters.is_empty() {
        table["parameters"] = toml_edit::value(parameters);
    }

    Ok(Item::Table(table))
}

fn default_value_for_field(field: &FieldSpec) -> Result<Option<Value>, String> {
    let Some(default_value) = field.default_value.as_deref() else {
        return Ok(None);
    };

    let value = match field.kind {
        FieldKind::String | FieldKind::Enum => Value::from(default_value),
        FieldKind::Integer => Value::from(
            default_value
                .parse::<i64>()
                .map_err(|_| format!("Default for '{}' is not an integer", field.key))?,
        ),
        FieldKind::Number => Value::from(
            default_value
                .parse::<f64>()
                .map_err(|_| format!("Default for '{}' is not a number", field.key))?,
        ),
        FieldKind::Boolean => Value::from(
            default_value
                .parse::<bool>()
                .map_err(|_| format!("Default for '{}' is not a boolean", field.key))?,
        ),
        FieldKind::Array | FieldKind::Object | FieldKind::JsonValue => {
            let item = toml_item_from_default_literal(default_value)?;
            item.into_value()
                .map_err(|_| format!("Default for '{}' is not an inline TOML value", field.key))?
        }
    };

    Ok(Some(value))
}

fn toml_item_from_default_literal(default_value: &str) -> Result<Item, String> {
    let snippet = format!("value = {}", default_value);
    let mut document = snippet.parse::<DocumentMut>().map_err(|error| {
        format!(
            "Failed to parse descriptor default '{}': {}",
            default_value, error
        )
    })?;

    Ok(document.remove("value").unwrap_or(Item::None))
}

fn ensure_document_table<'a>(
    document: &'a mut DocumentMut,
    key: &str,
) -> Result<&'a mut Item, String> {
    if document.get(key).is_none() {
        document[key] = Item::Table(Table::new());
    }

    document
        .get_mut(key)
        .filter(|item| item.as_table().is_some())
        .ok_or_else(|| format!("'{}' is not editable as a table", key))
}

fn ensure_child_table<'a>(parent: &'a mut Item, key: &str) -> Result<&'a mut Item, String> {
    if parent.get(key).is_none() {
        parent[key] = Item::Table(Table::new());
    }

    parent
        .get_mut(key)
        .filter(|item| item.as_table().is_some())
        .ok_or_else(|| format!("'{}' is not editable as a table", key))
}

fn insert_named_node(
    parent: &mut Item,
    node_name: &str,
    stage: Item,
    label: &str,
) -> Result<(), String> {
    if parent.get(node_name).is_some() {
        return Err(format!("A {} named '{}' already exists", label, node_name));
    }

    parent[node_name] = stage;
    Ok(())
}

fn validate_node_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Node name cannot be empty".to_string());
    }

    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return Err(
            "Node names may only contain letters, numbers, underscores, and hyphens".to_string(),
        );
    }

    Ok(())
}

fn default_channel_name(node_name: &str) -> String {
    format!("{}_data", node_name.replace('-', "_"))
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
            .map(|(key, value)| {
                Ok(format!(
                    "{} = {}",
                    toml_key(key),
                    toml_literal_from_json(value)?
                ))
            })
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

fn stage_item_mut<'a>(
    document: &'a mut DocumentMut,
    node_id: &str,
) -> Result<&'a mut Item, String> {
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
        .plugin(tauri_plugin_dialog::init())
        .manage(PipelineRuntime::default())
        .invoke_handler(tauri::generate_handler![
            load_graph,
            load_config_text,
            save_config_text,
            save_config_as,
            copy_config_to_workspace,
            start_pipeline,
            stop_pipeline,
            pipeline_runtime_state,
            list_workspace_configs,
            pick_config_file,
            pick_workspace_folder,
            pick_save_config_path,
            add_node,
            add_node_draft,
            delete_node,
            delete_node_draft,
            connect_nodes,
            connect_nodes_draft,
            disconnect_edge,
            disconnect_edge_draft,
            list_example_configs,
            list_processor_descriptors,
            update_node_field,
            update_node_field_draft,
            update_node_parameter_json,
            update_node_parameter_json_draft,
            update_node_parameter,
            update_node_parameter_draft
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

        update_existing_field(
            &mut document,
            "input:sensor",
            "concurrency.type",
            "pipeline",
        )
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
            parsed["pipelines"]["rules"]["stages"]["filter"]["parameters"]["rules"][0]["condition"]
                ["field_path"],
            "temperature"
        );
    }

    #[test]
    fn adds_nested_rules_parameter_to_inline_defaults() {
        let mut document = parse_document(
            r#"
[pipelines.rules]
description = "Rule pipeline"

[pipelines.rules.stages.filter]
type = "rule"
inputs = ["raw_data"]
output = "filtered_data"
parameters = { error_strategy = "continue" }
"#,
        );
        let value = serde_json::json!([
            {
                "condition": { "field_path": "device_id", "operation": "equals", "value": "imu" },
                "actions": [
                    { "type": "pass_through" }
                ],
                "else_actions": []
            }
        ]);

        update_json_parameter(
            &mut document,
            "pipeline:rules.stage:filter",
            "rules",
            &value,
        )
        .expect("missing nested rule parameter is added");

        let edited = document.to_string();

        let parsed: serde_json::Value = toml::from_str::<toml::Value>(&edited)
            .expect("edited TOML parses")
            .try_into()
            .expect("edited TOML converts to JSON");

        assert!(edited.contains("error_strategy = \"continue\""));
        assert!(edited.contains("device_id"));
        assert_eq!(
            parsed["pipelines"]["rules"]["stages"]["filter"]["parameters"]["rules"][0]["condition"]
                ["field_path"],
            "device_id"
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
    fn adds_input_node_with_output_and_descriptor_defaults() {
        let mut document = parse_document(
            r#"
[outputs.console]
type = "console"
"#,
        );
        let descriptor = processor_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.type_name == "simulated")
            .expect("simulated descriptor exists");

        add_node_to_document(&mut document, &descriptor, "new_sensor", None)
            .expect("input node is added");

        let edited = document.to_string();
        assert!(edited.contains("[inputs.new_sensor]"));
        assert!(edited.contains("type = \"simulated\""));
        assert!(edited.contains("output = \"new_sensor_data\""));
        assert!(edited.contains("interval_ms = 1000"));
        assert!(edited.contains("distribution = \"uniform\""));
        assert!(!edited.contains("inputs = []"));
    }

    #[test]
    fn adds_pipeline_stage_to_named_pipeline() {
        let mut document = parse_document(
            r#"
[pipelines.rules]
description = "Rules"
"#,
        );
        let descriptor = processor_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.type_name == "rule")
            .expect("rule descriptor exists");

        add_node_to_document(&mut document, &descriptor, "new_filter", Some("rules"))
            .expect("pipeline stage is added");

        let edited = document.to_string();
        assert!(edited.contains("[pipelines.rules.stages.new_filter]"));
        assert!(edited.contains("type = \"rule\""));
        assert!(edited.contains("inputs = []"));
        assert!(edited.contains("output = \"new_filter_data\""));
        assert!(edited.contains("rules = ["));

        let config = toml::from_str::<Config>(&edited).expect("edited config parses");
        let stage = config
            .pipelines
            .get("rules")
            .and_then(|pipeline| pipeline.stages.get("new_filter"))
            .expect("new rule stage exists")
            .clone();
        create_processor("rule", stage).expect("descriptor-created rule stage can be built");
    }

    #[test]
    fn adds_fusion_aggregator_with_descriptor_defaults() {
        let mut document = parse_document(
            r#"
[pipelines.default_pipeline]
description = "Default"
"#,
        );
        let descriptor = processor_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.type_name == "fusion")
            .expect("fusion descriptor exists");

        add_node_to_document(&mut document, &descriptor, "joiner", Some("default_pipeline"))
            .expect("fusion stage is added");

        let edited = document.to_string();
        assert!(edited.contains("[pipelines.default_pipeline.stages.joiner]"));
        assert!(edited.contains("type = \"fusion\""));
        assert!(edited.contains("mode = \"merge_objects\""));
        assert!(edited.contains("conflict_strategy = \"prefix\""));
        assert!(edited.contains("join_window_ms = 25"));

        let config = toml::from_str::<Config>(&edited).expect("edited config parses");
        let stage = config
            .pipelines
            .get("default_pipeline")
            .and_then(|pipeline| pipeline.stages.get("joiner"))
            .expect("new fusion stage exists")
            .clone();
        create_processor("fusion", stage).expect("descriptor-created fusion stage can be built");
    }

    #[test]
    fn popcorn_example_processors_are_constructible() {
        let config = load_config(Path::new("../../config/examples/config_popcorn.toml"))
            .expect("popcorn example loads");

        for (name, stage) in &config.inputs {
            create_processor(&stage.r#type, stage.clone())
                .unwrap_or_else(|error| panic!("input '{}' can be built: {}", name, error));
        }

        for (pipeline_name, pipeline) in &config.pipelines {
            for (stage_name, stage) in &pipeline.stages {
                create_processor(&stage.r#type, stage.clone()).unwrap_or_else(|error| {
                    panic!(
                        "pipeline stage '{}.{}' can be built: {}",
                        pipeline_name, stage_name, error
                    )
                });
            }
        }

        for (name, stage) in &config.outputs {
            create_processor(&stage.r#type, stage.clone())
                .unwrap_or_else(|error| panic!("output '{}' can be built: {}", name, error));
        }
    }

    #[test]
    fn preflight_reports_real_processor_construction_error() {
        let config = toml::from_str::<Config>(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[pipelines.main]
description = "Main"

[pipelines.main.stages.detector]
type = "rule"
inputs = ["raw_data"]
output = "detector_data"
parameters = { error_strategy = "continue", rules = [{ condition = { field_path = "value", operation = ">", value = 10 }, actions = [{ type = "keep_only_fields", field_paths = "[]" }], else_actions = [] }] }

[outputs.console]
type = "console"
inputs = ["detector_data"]
"#,
        )
        .expect("bad runtime config still parses structurally");

        let error = preflight_processors(&config)
            .expect_err("invalid processor parameters are rejected before launch");

        assert!(error.contains("Runtime preflight failed for stage 'main.detector'"));
        assert!(error.contains("processor 'rule' could not be created"));
        assert!(error.contains("invalid rules parameter"));
    }

    #[test]
    fn rejects_duplicate_added_node() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
"#,
        );
        let descriptor = processor_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.type_name == "simulated")
            .expect("simulated descriptor exists");

        let error = add_node_to_document(&mut document, &descriptor, "sensor", None)
            .expect_err("duplicate input node is rejected");

        assert!(error.contains("already exists"));
    }

    #[test]
    fn deletes_producer_node_and_removes_downstream_inputs() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[pipelines.rules.stages.filter]
type = "rule"
inputs = ["raw_data", "other_data"]
output = "filtered_data"

[outputs.console]
type = "console"
inputs = ["raw_data", "filtered_data"]
"#,
        );

        delete_node_from_document(&mut document, "input:sensor").expect("input node is deleted");
        remove_channel_from_all_inputs(&mut document, "raw_data")
            .expect("downstream inputs are cleaned");

        let edited = document.to_string();
        assert!(!edited.contains("[inputs.sensor]"));
        assert!(!edited.contains("\"raw_data\""));
        assert!(edited.contains("\"other_data\""));
        assert!(edited.contains("\"filtered_data\""));
    }

    #[test]
    fn deletes_output_node_without_touching_producers() {
        let mut document = parse_document(
            r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[outputs.console]
type = "console"
inputs = ["raw_data"]
"#,
        );

        delete_node_from_document(&mut document, "output:console").expect("output node is deleted");

        let edited = document.to_string();
        assert!(edited.contains("[inputs.sensor]"));
        assert!(edited.contains("output = \"raw_data\""));
        assert!(!edited.contains("[outputs.console]"));
    }

    #[test]
    fn writable_config_path_requires_toml_extension() {
        let error =
            writable_config_path("pipeline.txt").expect_err("non TOML save path is rejected");

        assert!(error.contains(".toml"));
    }

    #[test]
    fn recursively_lists_workspace_toml_configs() {
        let workspace = unique_temp_workspace_path();
        let nested = workspace.join("nested");
        fs::create_dir_all(&nested).expect("workspace folders are created");
        fs::write(workspace.join("root.toml"), "").expect("root toml is written");
        fs::write(nested.join("child.toml"), "").expect("nested toml is written");
        fs::write(nested.join("notes.txt"), "").expect("non toml is written");

        let mut configs = Vec::new();
        collect_toml_files(&workspace, &mut configs).expect("workspace configs are listed");
        let _ = fs::remove_dir_all(&workspace);

        assert_eq!(configs.len(), 2);
        assert!(configs.iter().any(|path| path.ends_with("root.toml")));
        assert!(configs.iter().any(|path| path.ends_with("child.toml")));
        assert!(!configs.iter().any(|path| path.ends_with("notes.txt")));
    }

    #[test]
    fn copies_config_into_workspace() {
        let workspace = unique_temp_workspace_path();
        let source = unique_temp_config_path();
        let content = r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"

[outputs.console]
type = "console"
inputs = ["raw_data"]
"#;
        fs::write(&source, content).expect("source config is written");

        let copied = copy_config_to_workspace_path(
            &workspace.to_string_lossy(),
            &source.to_string_lossy(),
            content,
        )
        .expect("config is copied into workspace");

        assert_eq!(copied.parent(), Some(workspace.as_path()));
        assert_eq!(copied.file_name(), source.file_name());
        assert_eq!(fs::read_to_string(&copied).expect("copied file is readable"), content);

        let _ = fs::remove_file(&source);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn copy_into_workspace_refuses_to_overwrite_existing_config() {
        let workspace = unique_temp_workspace_path();
        fs::create_dir_all(&workspace).expect("workspace folder is created");
        let source = unique_temp_config_path();
        let target = workspace.join(source.file_name().expect("source has a file name"));
        let content = r#"
[inputs.sensor]
type = "simulated"
output = "raw_data"
"#;
        fs::write(&source, content).expect("source config is written");
        fs::write(&target, content).expect("existing workspace config is written");

        let error = copy_config_to_workspace_path(
            &workspace.to_string_lossy(),
            &source.to_string_lossy(),
            content,
        )
        .expect_err("existing workspace config is not overwritten");

        assert!(error.contains("already contains"));

        let _ = fs::remove_file(&source);
        let _ = fs::remove_dir_all(&workspace);
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

        let number_error =
            update_existing_parameter(&mut document, "input:sensor", "interval_ms", "not-a-number")
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

    fn unique_temp_workspace_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after UNIX epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "liminal-gui-workspace-test-{}-{}",
            std::process::id(),
            nanos
        ))
    }
}
