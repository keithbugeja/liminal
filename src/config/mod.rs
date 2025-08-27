///! Configuration Module

pub mod loader;
pub mod types;
pub mod validation;
pub mod field;
pub mod params;
pub mod traits;

pub use field::FieldConfig;
pub use traits::ProcessorConfig;

pub use loader::{load_config};
pub use params::{extract_param, extract_field_params};
pub use types::{ Config, StageConfig, TimingConfig };
pub use validation::validate_config;
