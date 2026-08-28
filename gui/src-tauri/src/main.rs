use liminal::config::{load_config, ResolvedPipelineGraph};
use std::fs;
use std::path::{Path, PathBuf};

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_graph, list_example_configs])
        .run(tauri::generate_context!())
        .expect("error while running Liminal Pipeline GUI");
}
