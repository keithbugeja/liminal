use serde_json::Value;
use std::time::{SystemTime, Duration};

/// Timing metadata for messages in the processing pipeline
#[derive(Debug, Clone)]
pub struct TimingInfo {
    /// When the event actually occurred (event time)
    pub event_time: SystemTime,
    
    /// When the message was ingested into the system
    pub ingestion_time: SystemTime,
    
    /// Processing deadline for this message (optional)
    pub processing_deadline: Option<SystemTime>,
    
    /// Current watermark for this event stream (optional)
    pub watermark: Option<SystemTime>,
    
    /// Sequence ID for ordering within a partition/key (optional)
    pub sequence_id: Option<u64>,
    
    /// Trace ID for debugging and correlation (optional)
    pub trace_id: Option<String>,
}

impl TimingInfo {
    /// Create timing info with current time as both event and ingestion time
    pub fn now() -> Self {
        let now = SystemTime::now();
        Self {
            event_time: now,
            ingestion_time: now,
            processing_deadline: None,
            watermark: None,
            sequence_id: None,
            trace_id: None,
        }
    }
    
    /// Create timing info with explicit event time
    pub fn with_event_time(event_time: SystemTime) -> Self {
        // Use same time for ingestion as event time for simulated data
        // In real scenarios, ingestion_time would be when the message enters the system
        Self {
            event_time,
            ingestion_time: event_time,
            processing_deadline: None,
            watermark: None,
            sequence_id: None,
            trace_id: None,
        }
    }
    
    /// Create timing info with explicit event and ingestion times
    pub fn with_times(event_time: SystemTime, ingestion_time: SystemTime) -> Self {
        Self {
            event_time,
            ingestion_time,
            processing_deadline: None,
            watermark: None,
            sequence_id: None,
            trace_id: None,
        }
    }
    
    /// Get processing latency (ingestion_time - event_time)
    pub fn processing_latency(&self) -> Duration {
        self.ingestion_time.duration_since(self.event_time)
            .unwrap_or(Duration::ZERO)
    }
    
    /// Check if message has exceeded its processing deadline
    pub fn is_deadline_exceeded(&self) -> bool {
        if let Some(deadline) = self.processing_deadline {
            SystemTime::now() > deadline
        } else {
            false
        }
    }
    
    /// Get time until processing deadline
    pub fn time_until_deadline(&self) -> Option<Duration> {
        self.processing_deadline.and_then(|deadline| {
            deadline.duration_since(SystemTime::now()).ok()
        })
    }
    
    /// Check if this message is late based on watermark
    pub fn is_late(&self) -> bool {
        if let Some(watermark) = self.watermark {
            self.event_time < watermark
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub source: String,
    pub topic: String,
    pub payload: Value,
    
    /// Legacy timestamp field (ingestion time in milliseconds since epoch)
    /// Kept for backwards compatibility
    pub timestamp: u64,
    
    /// Enhanced timing information
    pub timing: TimingInfo,
}

impl Message {
    /// Create a new message with current time as both event and ingestion time
    pub fn new(source: &str, topic: &str, payload: Value) -> Self {
        let timing = TimingInfo::now();
        let timestamp = timing.ingestion_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            source: source.to_string(),
            topic: topic.to_string(),
            payload,
            timestamp,
            timing,
        }
    }
    
    /// Create a new message with explicit event time
    pub fn new_with_event_time(source: &str, topic: &str, payload: Value, event_time: SystemTime) -> Self {
        let timing = TimingInfo::with_event_time(event_time);
        let timestamp = timing.ingestion_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            source: source.to_string(),
            topic: topic.to_string(),
            payload,
            timestamp,
            timing,
        }
    }
    
    /// Set processing deadline for this message
    pub fn with_deadline(mut self, deadline: SystemTime) -> Self {
        self.timing.processing_deadline = Some(deadline);
        self
    }
    
    /// Set watermark for this message
    pub fn with_watermark(mut self, watermark: SystemTime) -> Self {
        self.timing.watermark = Some(watermark);
        self
    }
    
    /// Set sequence ID for ordering
    pub fn with_sequence_id(mut self, sequence_id: u64) -> Self {
        self.timing.sequence_id = Some(sequence_id);
        self
    }
    
    /// Set trace ID for debugging
    pub fn with_trace_id(mut self, trace_id: String) -> Self {
        self.timing.trace_id = Some(trace_id);
        self
    }
    
    /// Update timing info when message is processed by a stage
    pub fn mark_processed_by(mut self, stage_name: &str) -> Self {
        self.source = stage_name.to_string();
        // Could add processing history here if needed
        self
    }
    
    /// Check if message should be processed based on timing constraints
    pub fn should_process(&self) -> bool {
        !self.timing.is_deadline_exceeded()
    }
}
