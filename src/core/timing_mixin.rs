/// Timing mixin for processors that provides common timing functionality
/// This encapsulates all timing-related state and behavior that every processor needs

use crate::core::timing::{TimingConfig, WatermarkManager, TimingHelpers};
use crate::core::message::Message;
use tokio::time::Duration;
use std::time::SystemTime;

/// Mixin struct that encapsulates all timing-related functionality
/// This eliminates duplication across all processor implementations
#[derive(Debug)]
pub struct TimingMixin {
    /// Internal timing configuration (converted from TOML config)
    timing_config: TimingConfig,
    /// Watermark manager for this processor
    watermark_manager: WatermarkManager,
    /// Sequence counter for message ordering
    sequence_counter: u64,
    /// Optional TOML timing configuration (for reference/debugging)
    source_config: Option<crate::config::TimingConfig>,
}

impl TimingMixin {
    /// Create a new timing mixin from processor configuration 
    /// Takes the processed timing config from your processor's ProcessorConfig implementation
    /// This respects the ProcessorConfig::from_stage_config() pattern
    pub fn new(timing_config: Option<&crate::config::TimingConfig>) -> Self {
        let source_config = timing_config.cloned();
        
        // Convert TOML config to internal config
        let internal_timing_config = timing_config
            .map(|tc| tc.to_internal_config())
            .unwrap_or_default();
        
        let watermark_manager = WatermarkManager::new(internal_timing_config.clone());
        
        Self {
            timing_config: internal_timing_config,
            watermark_manager,
            sequence_counter: 0,
            source_config,
        }
    }
    
    /// Get the next sequence number and increment the counter
    pub fn next_sequence_id(&mut self) -> u64 {
        self.sequence_counter += 1;
        self.sequence_counter
    }
    
    /// Get current sequence counter value without incrementing
    pub fn current_sequence_id(&self) -> u64 {
        self.sequence_counter
    }
    
    /// Create a message with timing semantics applied
    pub fn create_message_with_timing(
        &mut self,
        source: &str,
        topic: &str,
        payload: serde_json::Value,
        event_time: SystemTime,
    ) -> Message {
        let sequence_id = self.next_sequence_id();
        
        let mut message = Message::new_with_event_time(source, topic, payload, event_time);
        message = message.with_sequence_id(sequence_id);
        
        // Add processing deadline if configured
        if let Some(ref source_config) = self.source_config {
            if let Some(timeout_ms) = source_config.processing_timeout_ms {
                message = TimingHelpers::add_processing_deadline(
                    message,
                    Duration::from_millis(timeout_ms),
                );
            }
        }
        
        // Update watermark
        let watermark = self.watermark_manager.update_watermark(&message);
        TimingHelpers::update_message_watermark(message, watermark)
    }
    
    /// Create a message with event time extracted from payload if configured
    pub fn create_message_with_event_time_extraction(
        &mut self,
        source: &str,
        topic: &str,
        payload: serde_json::Value,
        fallback_event_time: SystemTime,
    ) -> Message {
        let event_time = if let Some(ref source_config) = self.source_config {
            if let Some(event_time_field) = &source_config.event_time_field {
                // Extract event time from payload (fallback to provided time if not found)
                TimingHelpers::extract_event_time(&payload, event_time_field)
            } else {
                fallback_event_time
            }
        } else {
            fallback_event_time
        };
        
        self.create_message_with_timing(source, topic, payload, event_time)
    }
    
    /// Update watermark for an existing message
    pub fn update_message_watermark(&mut self, message: Message) -> Message {
        let watermark = self.watermark_manager.update_watermark(&message);
        TimingHelpers::update_message_watermark(message, watermark)
    }
    
    /// Check if a message should be dropped due to timing constraints
    pub fn should_drop_message(&self, message: &Message) -> bool {
        TimingHelpers::should_drop_message(message, &self.timing_config)
    }
    
    /// Get timing configuration (for TimingProcessor trait)
    pub fn timing_config(&self) -> &TimingConfig {
        &self.timing_config
    }
    
    /// Get mutable watermark manager (for TimingProcessor trait)
    pub fn watermark_manager(&mut self) -> &mut WatermarkManager {
        &mut self.watermark_manager
    }
    
    /// Get source timing configuration (for debugging/inspection)
    pub fn source_timing_config(&self) -> Option<&crate::config::TimingConfig> {
        self.source_config.as_ref()
    }
}

/// Trait for processors that use the timing mixin
/// This provides a standard interface for timing operations
pub trait WithTimingMixin {
    /// Get the timing mixin
    fn timing_mixin(&self) -> &TimingMixin;
    
    /// Get the mutable timing mixin
    fn timing_mixin_mut(&mut self) -> &mut TimingMixin;
    
    /// Convenience method to create a message with timing
    fn create_timed_message(
        &mut self,
        source: &str,
        topic: &str,
        payload: serde_json::Value,
        event_time: SystemTime,
    ) -> Message {
        self.timing_mixin_mut().create_message_with_timing(source, topic, payload, event_time)
    }
    
    /// Convenience method to create a message with event time extraction
    fn create_message_with_extraction(
        &mut self,
        source: &str,
        topic: &str,
        payload: serde_json::Value,
        fallback_event_time: SystemTime,
    ) -> Message {
        self.timing_mixin_mut().create_message_with_event_time_extraction(
            source, topic, payload, fallback_event_time
        )
    }
}

/// Blanket implementation of TimingProcessor for anything that has TimingMixin
impl<T: WithTimingMixin> crate::core::timing::TimingProcessor for T {
    fn timing_config(&self) -> &TimingConfig {
        self.timing_mixin().timing_config()
    }
    
    fn watermark_manager(&mut self) -> &mut WatermarkManager {
        self.timing_mixin_mut().watermark_manager()
    }
}
