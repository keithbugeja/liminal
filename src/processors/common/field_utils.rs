use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldPathSegment {
    Key(String),
    Index(usize),
}

/// Utility functions for working with JSON field paths and values
pub struct FieldUtils;

impl FieldUtils {
    /// Parse a field path into object-key and array-index segments.
    ///
    /// Supported grammar:
    /// - object keys separated with `.`, e.g. `device.id`
    /// - array indexes after any segment, e.g. `items[0].value`
    /// - root array indexes, e.g. `[0].value`
    fn parse_field_path(field_path: &str) -> Result<Vec<FieldPathSegment>> {
        if field_path.trim().is_empty() {
            return Err(anyhow!("Empty field path"));
        }

        let chars: Vec<char> = field_path.chars().collect();
        let mut segments = Vec::new();
        let mut key = String::new();
        let mut index = 0;
        let mut expect_segment = true;

        while index < chars.len() {
            match chars[index] {
                '.' => {
                    if expect_segment {
                        return Err(anyhow!(
                            "Invalid empty field path segment in '{field_path}'"
                        ));
                    }
                    if !key.is_empty() {
                        segments.push(FieldPathSegment::Key(std::mem::take(&mut key)));
                    }
                    expect_segment = true;
                    index += 1;
                }
                '[' => {
                    if !key.is_empty() {
                        segments.push(FieldPathSegment::Key(std::mem::take(&mut key)));
                    }

                    index += 1;
                    let start = index;
                    while index < chars.len() && chars[index] != ']' {
                        index += 1;
                    }

                    if index >= chars.len() {
                        return Err(anyhow!("Unclosed array index in field path '{field_path}'"));
                    }

                    let raw_index: String = chars[start..index].iter().collect();
                    if raw_index.is_empty() {
                        return Err(anyhow!("Empty array index in field path '{field_path}'"));
                    }
                    let array_index = raw_index.parse::<usize>().map_err(|_| {
                        anyhow!("Invalid array index '{raw_index}' in field path '{field_path}'")
                    })?;

                    segments.push(FieldPathSegment::Index(array_index));
                    expect_segment = false;
                    index += 1;

                    if index < chars.len() && chars[index] != '.' && chars[index] != '[' {
                        return Err(anyhow!(
                            "Unexpected character '{}' after array index in field path '{}'",
                            chars[index],
                            field_path
                        ));
                    }
                }
                character => {
                    key.push(character);
                    expect_segment = false;
                    index += 1;
                }
            }
        }

        if !key.is_empty() {
            segments.push(FieldPathSegment::Key(key));
        } else if expect_segment {
            return Err(anyhow!("Field path '{field_path}' cannot end with '.'"));
        }

        Ok(segments)
    }

    /// Validate a field path without reading or mutating a payload.
    pub fn validate_field_path(field_path: &str) -> Result<()> {
        Self::parse_field_path(field_path).map(|_| ())
    }

    /// Extract a field value from a JSON payload using the Liminal field path grammar.
    ///
    /// # Arguments
    /// * `payload` - The JSON value to extract from
    /// * `field_path` - Path like "device.id", "accelerometer.x", or "items[0].value"
    ///
    /// # Examples
    /// ```
    /// let json = serde_json::json!({"device": {"id": "esp32-001"}});
    /// let value = FieldUtils::extract_field_value(&json, "device.id");
    /// assert_eq!(value, Some(&serde_json::json!("esp32-001")));
    /// ```
    pub fn extract_field_value<'a>(payload: &'a Value, field_path: &str) -> Option<&'a Value> {
        let segments = Self::parse_field_path(field_path).ok()?;
        let mut current = payload;

        for segment in segments {
            current = match segment {
                FieldPathSegment::Key(key) => current.get(key)?,
                FieldPathSegment::Index(index) => current.get(index)?,
            };
        }

        Some(current)
    }

    /// Set a field value in a JSON payload using the Liminal field path grammar.
    /// Creates nested objects or arrays as needed.
    ///
    /// # Arguments
    /// * `payload` - The JSON value to modify (must be mutable)
    /// * `field_path` - Path like "device.id", "accelerometer.x", or "items[0].value"
    /// * `value` - The value to set
    pub fn set_field_value(payload: &mut Value, field_path: &str, value: Value) -> Result<()> {
        let segments = Self::parse_field_path(field_path)?;

        // Ensure object-key paths have an object root, preserving root arrays for `[0]` paths.
        match segments.first() {
            Some(FieldPathSegment::Key(_)) if !payload.is_object() => {
                *payload = Value::Object(Map::new());
            }
            Some(FieldPathSegment::Index(_)) if !payload.is_array() => {
                *payload = Value::Array(Vec::new());
            }
            _ => {}
        }

        let mut current = payload;

        // Navigate to the parent of the target field
        for (position, segment) in segments[..segments.len() - 1].iter().enumerate() {
            let next_segment = &segments[position + 1];
            current = match segment {
                FieldPathSegment::Key(key) => {
                    let obj = current
                        .as_object_mut()
                        .ok_or_else(|| anyhow!("Cannot navigate through non-object at '{key}'"))?;

                    if !obj.contains_key(key) {
                        obj.insert(key.clone(), empty_container_for(next_segment));
                    }

                    let value = obj.get_mut(key).unwrap();
                    if value.is_null() {
                        *value = empty_container_for(next_segment);
                    }
                    value
                }
                FieldPathSegment::Index(index) => {
                    let array = current.as_array_mut().ok_or_else(|| {
                        anyhow!("Cannot navigate array index [{index}] through non-array value")
                    })?;

                    if *index >= array.len() {
                        array.resize(*index + 1, Value::Null);
                    }

                    let value = array.get_mut(*index).unwrap();
                    if value.is_null() {
                        *value = empty_container_for(next_segment);
                    }
                    value
                }
            };
        }

        match segments.last().unwrap() {
            FieldPathSegment::Key(key) => {
                let obj = current.as_object_mut().ok_or_else(|| {
                    anyhow!("Cannot set object field '{key}' on non-object value")
                })?;
                obj.insert(key.clone(), value);
            }
            FieldPathSegment::Index(index) => {
                let array = current.as_array_mut().ok_or_else(|| {
                    anyhow!("Cannot set array index [{index}] on non-array value")
                })?;
                if *index >= array.len() {
                    array.resize(index + 1, Value::Null);
                }
                let slot = array.get_mut(*index).unwrap();
                *slot = value;
            }
        }

        Ok(())
    }

    /// Remove a field from a JSON payload using the Liminal field path grammar.
    ///
    /// # Arguments
    /// * `payload` - The JSON value to modify (must be mutable)
    /// * `field_path` - Path like "device.id", "accelerometer.x", or "items[0].value"
    pub fn remove_field_value(payload: &mut Value, field_path: &str) -> Result<()> {
        let segments = Self::parse_field_path(field_path)?;

        // Navigate to parent object
        let mut current = payload;
        for segment in &segments[..segments.len() - 1] {
            current = match segment {
                FieldPathSegment::Key(key) => match current.get_mut(key) {
                    Some(value) => value,
                    None => return Ok(()),
                },
                FieldPathSegment::Index(index) => match current.get_mut(*index) {
                    Some(value) => value,
                    None => return Ok(()),
                },
            };
        }

        match segments.last().unwrap() {
            FieldPathSegment::Key(key) => {
                if let Some(obj) = current.as_object_mut() {
                    obj.remove(key);
                }
            }
            FieldPathSegment::Index(index) => {
                if let Some(array) = current.as_array_mut() {
                    if *index < array.len() {
                        array.remove(*index);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a field exists in a JSON payload using the Liminal field path grammar.
    ///
    /// # Arguments
    /// * `payload` - The JSON value to check
    /// * `field_path` - Path like "device.id", "accelerometer.x", or "items[0].value"
    pub fn field_exists(payload: &Value, field_path: &str) -> bool {
        Self::extract_field_value(payload, field_path).is_some()
    }
}

fn empty_container_for(segment: &FieldPathSegment) -> Value {
    match segment {
        FieldPathSegment::Key(_) => Value::Object(Map::new()),
        FieldPathSegment::Index(_) => Value::Array(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_dot_and_array_paths() {
        let path = FieldUtils::parse_field_path("items[0].device.id").expect("valid path");

        assert_eq!(
            path,
            vec![
                FieldPathSegment::Key("items".to_string()),
                FieldPathSegment::Index(0),
                FieldPathSegment::Key("device".to_string()),
                FieldPathSegment::Key("id".to_string()),
            ]
        );
    }

    #[test]
    fn rejects_invalid_paths() {
        for path in ["", ".", "items.", "items[]", "items[abc]", "items[0"] {
            assert!(
                FieldUtils::validate_field_path(path).is_err(),
                "{path} should be invalid"
            );
        }
    }

    #[test]
    fn extracts_array_values() {
        let payload = json!({
            "items": [
                { "value": 10 },
                { "value": 20 }
            ]
        });

        assert_eq!(
            FieldUtils::extract_field_value(&payload, "items[1].value"),
            Some(&json!(20))
        );
    }

    #[test]
    fn sets_existing_array_values() {
        let mut payload = json!({
            "items": [
                { "value": 10 }
            ]
        });

        FieldUtils::set_field_value(&mut payload, "items[0].value", json!(42))
            .expect("array element can be updated");

        assert_eq!(payload["items"][0]["value"], json!(42));
    }

    #[test]
    fn set_creates_missing_array_containers() {
        let mut payload = json!({ "items": [] });

        FieldUtils::set_field_value(&mut payload, "items[0].value", json!(42))
            .expect("arrays are created from path shape");

        assert_eq!(payload["items"][0]["value"], json!(42));
    }

    #[test]
    fn removes_array_values() {
        let mut payload = json!({ "items": ["a", "b", "c"] });

        FieldUtils::remove_field_value(&mut payload, "items[1]").expect("array element removed");

        assert_eq!(payload["items"], json!(["a", "c"]));
    }
}
