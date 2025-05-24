use super::types::Config;
use std::fs;
use std::path::Path;
use toml;

/// Load configuration from a TOML file
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// Load configuration from a string
pub fn load_config_from_string(content: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let config: Config = toml::from_str(content)?;
    Ok(config)
}