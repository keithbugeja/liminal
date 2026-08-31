use liminal::config::Config;
use std::fs;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::FilePath;

pub(crate) fn resolve_config_path(path: &str) -> Result<PathBuf, String> {
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

pub(crate) fn writable_config_path(path: &str) -> Result<PathBuf, String> {
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

pub(crate) fn collect_toml_files(folder: &Path, configs: &mut Vec<String>) -> Result<(), String> {
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

pub(crate) fn copy_config_to_workspace_path(
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

pub(crate) fn writable_directory_path(path: &str) -> Result<PathBuf, String> {
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

pub(crate) fn dialog_path_to_string(path: FilePath) -> Result<String, String> {
    path.simplified()
        .into_path()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| format!("Selected path is not a filesystem path: {}", error))
}

pub(crate) async fn wait_for_dialog_path(
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

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("src-tauri is nested under gui")
        .to_path_buf()
}

pub(crate) fn relative_to_repo(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
