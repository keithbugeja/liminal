pub mod processor;
pub mod factory;
pub mod common;

pub mod input;
pub mod output;
pub mod transform;
pub mod aggregator;

pub use processor::Processor;
// pub use input::*;
// pub use transform::*;
// pub use aggregator::*;
// pub use output::*;

pub use factory::create_processor;
