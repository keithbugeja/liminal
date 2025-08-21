use anyhow::{anyhow, Result};
use serde_json::{Value, Map};

/// Utility functions for working with JSON field paths and values
pub struct FieldUtils;

impl FieldUtils {
    /// Extract a field value from a JSON payload using dot notation path
    /// 
    /// # Arguments
    /// * `payload` - The JSON value to extract from
    /// * `field_path` - Dot-separated path like "device.id" or "accelerometer.x"
    /// 
    /// # Examples
    /// ```
    /// let json = serde_json::json!({"device": {"id": "esp32-001"}});
    /// let value = FieldUtils::extract_field_value(&json, "device.id");
    /// assert_eq!(value, Some(&serde_json::json!("esp32-001")));
    /// ```
    pub fn extract_field_value<'a>(payload: &'a Value, field_path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = payload;
        
        for part in parts {
            current = current.get(part)?;
        }
        
        Some(current)
    }

    /// Set a field value in a JSON payload using dot notation path
    /// Creates nested objects as needed
    /// 
    /// # Arguments
    /// * `payload` - The JSON value to modify (must be mutable)
    /// * `field_path` - Dot-separated path like "device.id" or "accelerometer.x"
    /// * `value` - The value to set
    pub fn set_field_value(payload: &mut Value, field_path: &str, value: Value) -> Result<()> {
        let parts: Vec<&str> = field_path.split('.').collect();
        
        if parts.is_empty() {
            return Err(anyhow!("Empty field path"));
        }
        
        // Ensure payload is an object
        if !payload.is_object() {
            *payload = Value::Object(Map::new());
        }
        
        let mut current = payload;
        
        // Navigate to the parent of the target field
        for part in &parts[..parts.len()-1] {
            if !current.is_object() {
                return Err(anyhow!("Cannot navigate through non-object value at '{}'", part));
            }
            
            let obj = current.as_object_mut().unwrap();
            
            if !obj.contains_key(*part) {
                obj.insert(part.to_string(), Value::Object(Map::new()));
            }
            
            current = obj.get_mut(*part).unwrap();
        }
        
        // Set the final field
        if let Some(obj) = current.as_object_mut() {
            obj.insert(parts[parts.len()-1].to_string(), value);
            Ok(())
        } else {
            Err(anyhow!("Cannot set field on non-object value"))
        }
    }

    /// Remove a field from a JSON payload using dot notation path
    /// 
    /// # Arguments
    /// * `payload` - The JSON value to modify (must be mutable)
    /// * `field_path` - Dot-separated path like "device.id" or "accelerometer.x"
    pub fn remove_field_value(payload: &mut Value, field_path: &str) -> Result<()> {
        let parts: Vec<&str> = field_path.split('.').collect();
        
        if parts.is_empty() {
            return Err(anyhow!("Empty field path"));
        }
        
        if parts.len() == 1 {
            // Simple case: remove from root object
            if let Some(obj) = payload.as_object_mut() {
                obj.remove(parts[0]);
            }
            return Ok(());
        }
        
        // Navigate to parent object
        let mut current = payload;
        for part in &parts[..parts.len()-1] {
            current = match current.get_mut(*part) {
                Some(value) => value,
                None => return Ok(()), // Path doesn't exist, nothing to remove
            };
        }
        
        // Remove the field from parent object
        if let Some(obj) = current.as_object_mut() {
            obj.remove(parts[parts.len()-1]);
        }
        
        Ok(())
    }

    /// Check if a field exists in a JSON payload using dot notation path
    /// 
    /// # Arguments
    /// * `payload` - The JSON value to check
    /// * `field_path` - Dot-separated path like "device.id" or "accelerometer.x"
    pub fn field_exists(payload: &Value, field_path: &str) -> bool {
        Self::extract_field_value(payload, field_path).is_some()
    }
}