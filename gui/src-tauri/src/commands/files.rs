use liminal::config::{load_config, Config, ResolvedPipelineGraph};
use liminal::processors::descriptor::{processor_descriptors, ProcessorDescriptor};
use std::fs;
use std::path::PathBuf;
use tauri_plugin_dialog::DialogExt;

use crate::paths::{
    collect_toml_files, copy_config_to_workspace_path, dialog_path_to_string, relative_to_repo,
    repo_root, resolve_config_path, wait_for_dialog_path, writable_config_path,
};

#[tauri::command]
pub(crate) fn load_graph(path: String) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let config = load_config(&resolved_path)
        .map_err(|error| format!("Failed to load '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
pub(crate) fn load_config_text(path: String) -> Result<String, String> {
    let resolved_path = resolve_config_path(&path)?;
    fs::read_to_string(&resolved_path)
        .map_err(|error| format!("Failed to read '{}': {}", resolved_path.display(), error))
}

#[tauri::command]
pub(crate) fn save_config_text(
    path: String,
    content: String,
) -> Result<ResolvedPipelineGraph, String> {
    let resolved_path = resolve_config_path(&path)?;
    let config = toml::from_str::<Config>(&content)
        .map_err(|error| format!("Edited TOML no longer parses: {}", error))?;

    fs::write(&resolved_path, content)
        .map_err(|error| format!("Failed to write '{}': {}", resolved_path.display(), error))?;

    Ok(ResolvedPipelineGraph::from_config(&config))
}

#[tauri::command]
pub(crate) fn save_config_as(
    path: String,
    content: String,
) -> Result<ResolvedPipelineGraph, String> {
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
pub(crate) fn copy_config_to_workspace(
    workspace_path: String,
    source_path: String,
    content: String,
) -> Result<String, String> {
    let target_path = copy_config_to_workspace_path(&workspace_path, &source_path, &content)?;
    Ok(relative_to_repo(&target_path))
}

#[tauri::command]
pub(crate) fn list_workspace_configs(path: String) -> Result<Vec<String>, String> {
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
pub(crate) async fn pick_config_file(window: tauri::Window) -> Result<Option<String>, String> {
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
pub(crate) async fn pick_workspace_folder(window: tauri::Window) -> Result<Option<String>, String> {
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
pub(crate) async fn pick_save_config_path(
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
pub(crate) fn list_example_configs() -> Result<Vec<String>, String> {
    let examples_dir = repo_root().join("config").join("examples");
    let mut configs = vec![relative_to_repo(
        &repo_root().join("config").join("config.toml"),
    )];

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
pub(crate) fn list_processor_descriptors() -> Vec<ProcessorDescriptor> {
    processor_descriptors()
}
