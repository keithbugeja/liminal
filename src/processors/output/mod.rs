pub mod console;
pub mod file;
pub mod mqtt;

pub use console::ConsoleOutputProcessor;
pub use file::FileOutputProcessor;
pub use mqtt::MqttOutputProcessor;