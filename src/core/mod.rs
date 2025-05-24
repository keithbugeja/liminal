pub mod channel;
pub mod message;
pub mod pipeline;
pub mod registry;
pub mod stage;
pub mod context;

pub use message::Message;
pub use stage::Stage;
pub use context::{ProcessingContext, OutputInfo};