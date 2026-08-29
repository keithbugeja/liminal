///! Configuration Module
pub mod field;
pub mod graph;
pub mod loader;
pub mod params;
pub mod traits;
pub mod types;
pub mod validation;

pub use field::FieldConfig;
pub use graph::ResolvedPipelineGraph;
pub use traits::ProcessorConfig;

pub use loader::load_config;
pub use params::{extract_field_params, extract_param};
pub use types::{Config, StageConfig, TimingConfig};
pub use validation::validate_config;
