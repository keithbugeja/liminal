use serde_json::Value;

/// Condition operations supported by processors
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionOperation {
    Equals,
    NotEquals,
    StartsWith,
    EndsWith,
    Contains,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

impl ConditionOperation {
    /// Parse a condition operation from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "equals" | "==" => Some(Self::Equals),
            "not_equals" | "!=" => Some(Self::NotEquals),
            "startswith" => Some(Self::StartsWith),
            "endswith" => Some(Self::EndsWith),
            "contains" => Some(Self::Contains),
            ">" => Some(Self::GreaterThan),
            ">=" => Some(Self::GreaterThanOrEqual),
            "<" => Some(Self::LessThan),
            "<=" => Some(Self::LessThanOrEqual),
            _ => None,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::NotEquals => "not_equals",
            Self::StartsWith => "startswith",
            Self::EndsWith => "endswith",
            Self::Contains => "contains",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
        }
    }
}

/// Utility functions for evaluating conditions against JSON values
pub struct ConditionEvaluator;

impl ConditionEvaluator {
    /// Evaluate a condition against a field value
    /// 
    /// # Arguments
    /// * `field_value` - The value to test
    /// * `operation` - The condition operation
    /// * `expected_value` - The value to compare against
    pub fn evaluate_condition(
        field_value: &Value,
        operation: &ConditionOperation,
        expected_value: &Value,
    ) -> bool {
        match operation {
            ConditionOperation::Equals => field_value == expected_value,
            ConditionOperation::NotEquals => field_value != expected_value,
            
            ConditionOperation::StartsWith => {
                if let (Value::String(field_str), Value::String(expected_str)) = (field_value, expected_value) {
                    field_str.starts_with(expected_str)
                } else {
                    false
                }
            }
            
            ConditionOperation::EndsWith => {
                if let (Value::String(field_str), Value::String(expected_str)) = (field_value, expected_value) {
                    field_str.ends_with(expected_str)
                } else {
                    false
                }
            }
            
            ConditionOperation::Contains => {
                if let (Value::String(field_str), Value::String(expected_str)) = (field_value, expected_value) {
                    field_str.contains(expected_str)
                } else {
                    false
                }
            }
            
            ConditionOperation::GreaterThan => {
                Self::compare_numbers(field_value, expected_value, |a, b| a > b)
            }
            
            ConditionOperation::GreaterThanOrEqual => {
                Self::compare_numbers(field_value, expected_value, |a, b| a >= b)
            }
            
            ConditionOperation::LessThan => {
                Self::compare_numbers(field_value, expected_value, |a, b| a < b)
            }
            
            ConditionOperation::LessThanOrEqual => {
                Self::compare_numbers(field_value, expected_value, |a, b| a <= b)
            }
        }
    }

    /// Parse and evaluate a filter-style condition string (like "startswith 'value'")
    /// This is for backward compatibility with FilterProcessor
    pub fn evaluate_filter_condition(field_value: &Value, condition_str: &str) -> bool {
        // String operations
        if let Some(value_str) = field_value.as_str() {
            // startswith condition
            if condition_str.starts_with("startswith '") && condition_str.ends_with("'") {
                let prefix = &condition_str[12..condition_str.len()-1];
                return value_str.starts_with(prefix);
            }
            
            // endswith condition
            if condition_str.starts_with("endswith '") && condition_str.ends_with("'") {
                let suffix = &condition_str[10..condition_str.len()-1];
                return value_str.ends_with(suffix);
            }
            
            // contains condition
            if condition_str.starts_with("contains '") && condition_str.ends_with("'") {
                let substring = &condition_str[10..condition_str.len()-1];
                return value_str.contains(substring);
            }
            
            // equals condition for strings
            if condition_str.starts_with("== '") && condition_str.ends_with("'") {
                let expected = &condition_str[4..condition_str.len()-1];
                return value_str == expected;
            }
            
            // not equals condition for strings
            if condition_str.starts_with("!= '") && condition_str.ends_with("'") {
                let expected = &condition_str[4..condition_str.len()-1];
                return value_str != expected;
            }
        }
        
        // Numeric operations
        if let Some(value_num) = field_value.as_f64() {
            // Greater than
            if condition_str.starts_with("> ") {
                if let Ok(threshold) = condition_str[2..].trim().parse::<f64>() {
                    return value_num > threshold;
                }
            }
            
            // Greater than or equal
            if condition_str.starts_with(">= ") {
                if let Ok(threshold) = condition_str[3..].trim().parse::<f64>() {
                    return value_num >= threshold;
                }
            }
            
            // Less than
            if condition_str.starts_with("< ") {
                if let Ok(threshold) = condition_str[2..].trim().parse::<f64>() {
                    return value_num < threshold;
                }
            }
            
            // Less than or equal
            if condition_str.starts_with("<= ") {
                if let Ok(threshold) = condition_str[3..].trim().parse::<f64>() {
                    return value_num <= threshold;
                }
            }
            
            // Equals for numbers
            if condition_str.starts_with("== ") {
                if let Ok(expected) = condition_str[3..].trim().parse::<f64>() {
                    return (value_num - expected).abs() < f64::EPSILON;
                }
            }
            
            // Not equals for numbers
            if condition_str.starts_with("!= ") {
                if let Ok(expected) = condition_str[3..].trim().parse::<f64>() {
                    return (value_num - expected).abs() >= f64::EPSILON;
                }
            }
        }
        
        // Boolean operations
        if let Some(value_bool) = field_value.as_bool() {
            if condition_str == "== true" {
                return value_bool;
            }
            if condition_str == "== false" {
                return !value_bool;
            }
        }
        
        // If no condition matched, return false
        false
    }

    /// Helper function to compare numeric values
    fn compare_numbers<F>(field_value: &Value, expected_value: &Value, comparator: F) -> bool
    where
        F: Fn(f64, f64) -> bool,
    {
        if let (Value::Number(field_num), Value::Number(expected_num)) = (field_value, expected_value) {
            let field_val = field_num.as_f64().unwrap_or(0.0);
            let expected_val = expected_num.as_f64().unwrap_or(0.0);
            comparator(field_val, expected_val)
        } else {
            false
        }
    }
}