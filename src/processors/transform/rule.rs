use crate::config::{ProcessorConfig, StageConfig, extract_param};
use crate::core::timing_mixin::{TimingMixin, WithTimingMixin};
use crate::core::{context::ProcessingContext, message::Message};
use crate::processors::common::condition_utils::{ConditionEvaluator, ConditionOperation};
use crate::processors::common::field_utils::FieldUtils;
use crate::processors::processor::Processor;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::collections::HashMap;
use tokio::select;
use tracing::{debug, error, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleConfig {
    pub rules: Vec<Rule>,
    #[serde(default = "default_error_strategy")]
    pub error_strategy: ErrorStrategy,
    #[serde(skip)]
    pub timing: Option<crate::config::TimingConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStrategy {
    Continue,   // Log error and continue processing
    Skip,       // Skip this action, continue with others
    Abort,      // Stop processing this message
    UseDefault, // Use default/fallback values on error
}

fn default_error_strategy() -> ErrorStrategy {
    ErrorStrategy::Continue
}

impl ProcessorConfig for RuleConfig {
    fn from_stage_config(config: &StageConfig) -> Result<Self> {
        let rules = extract_param::<Vec<Rule>>(&config.parameters, "rules", vec![]);
        if rules.is_empty() {
            return Err(anyhow!("rule_transformer requires at least one rule"));
        }

        let error_strategy = extract_param::<ErrorStrategy>(
            &config.parameters,
            "error_strategy",
            default_error_strategy(),
        );

        // Extract timing configuration
        let timing_config = config.timing.clone();

        Ok(Self {
            rules,
            error_strategy,
            timing : timing_config,
        })
    }
    fn validate(&self) -> Result<()> {
        if self.rules.is_empty() {
            return Err(anyhow!("At least one rule must be defined"));
        }

        for (i, rule) in self.rules.iter().enumerate() {
            if rule.condition.field_path.is_empty() {
                return Err(anyhow!("Rule {} has empty field_path", i));
            }
            if rule.condition.operation.is_empty() {
                return Err(anyhow!("Rule {} has empty operation", i));
            }

            // Validate operation is supported
            if ConditionOperation::from_str(&rule.condition.operation).is_none() {
                return Err(anyhow!(
                    "Rule {} has unsupported operation: '{}'",
                    i,
                    rule.condition.operation
                ));
            }

            if rule.actions.is_empty() {
                return Err(anyhow!("Rule {} has no actions", i));
            }

            // Validate actions
            for (j, action) in rule.actions.iter().enumerate() {
                RuleConfig::validate_action(action, &format!("Rule {}, Action {}", i, j))?;
            }

            // Validate else_actions
            for (j, action) in rule.else_actions.iter().enumerate() {
                RuleConfig::validate_action(action, &format!("Rule {}, Else Action {}", i, j))?;
            }
        }

        Ok(())
    }
}

impl RuleConfig {
    fn validate_action(action: &Action, context: &str) -> Result<()> {
        match action {
            Action::SetField { field_path, .. } => {
                if field_path.is_empty() {
                    return Err(anyhow!("{}: SetField has empty field_path", context));
                }
            }
            Action::RemoveField { field_path } => {
                if field_path.is_empty() {
                    return Err(anyhow!("{}: RemoveField has empty field_path", context));
                }
            }
            Action::CopyField {
                source_field,
                target_field,
            } => {
                if source_field.is_empty() {
                    return Err(anyhow!("{}: CopyField has empty source_field", context));
                }
                if target_field.is_empty() {
                    return Err(anyhow!("{}: CopyField has empty target_field", context));
                }
                if source_field == target_field {
                    return Err(anyhow!(
                        "{}: CopyField source and target are the same",
                        context
                    ));
                }
            }
            Action::RenameField {
                old_field,
                new_field,
            } => {
                if old_field.is_empty() {
                    return Err(anyhow!("{}: RenameField has empty old_field", context));
                }
                if new_field.is_empty() {
                    return Err(anyhow!("{}: RenameField has empty new_field", context));
                }
                if old_field == new_field {
                    return Err(anyhow!(
                        "{}: RenameField old and new field are the same",
                        context
                    ));
                }
            }
            Action::ComputeField {
                field_path,
                expression,
            } => {
                if field_path.is_empty() {
                    return Err(anyhow!("{}: ComputeField has empty field_path", context));
                }
                if expression.is_empty() {
                    return Err(anyhow!("{}: ComputeField has empty expression", context));
                }
                // Could add expression syntax validation here using evalexpr
            }
            Action::KeepOnlyFields { field_paths } => {
                // Empty field_paths is valid - it means "keep no fields" (clear everything)
                for field_path in field_paths {
                    if field_path.is_empty() {
                        return Err(anyhow!(
                            "{}: KeepOnlyFields contains empty field_path",
                            context
                        ));
                    }
                }
            }
            Action::DropMessage | Action::PassThrough => {
                // These actions have no parameters to validate
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub condition: Condition,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub else_actions: Vec<Action>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Condition {
    pub field_path: String,
    pub operation: String,
    pub value: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "set_field")]
    SetField { field_path: String, value: Value },
    #[serde(rename = "remove_field")]
    RemoveField { field_path: String },
    #[serde(rename = "copy_field")]
    CopyField {
        source_field: String,
        target_field: String,
    },
    #[serde(rename = "rename_field")]
    RenameField {
        old_field: String,
        new_field: String,
    },
    #[serde(rename = "compute_field")]
    ComputeField {
        field_path: String,
        expression: String,
    },
    #[serde(rename = "drop_message")]
    DropMessage,
    #[serde(rename = "pass_through")]
    PassThrough,
    #[serde(rename = "keep_only_fields")]
    KeepOnlyFields { field_paths: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum ActionPriority {
    PreCompute = 1, // ComputeField (pre-computation phase)
    Reset = 2,      // KeepOnlyFields (destructive reset)
    Transform = 3,  // SetField, CopyField, RenameField, RemoveField
    Finalize = 4,   // ComputeField (final computation phase)
    Control = 5,    // DropMessage, PassThrough
}

impl Action {
    fn priority(&self) -> ActionPriority {
        match self {
            Action::KeepOnlyFields { .. } => ActionPriority::Reset,
            Action::ComputeField { .. } => ActionPriority::Transform, // Will be handled specially
            Action::DropMessage | Action::PassThrough => ActionPriority::Control,
            _ => ActionPriority::Transform,
        }
    }

    fn needs_pre_computation(&self) -> bool {
        matches!(self, Action::ComputeField { .. })
    }
}

pub struct RuleProcessor {
    name: String,
    config: RuleConfig,
    timing: TimingMixin,
}

impl RuleProcessor {
    pub fn new(name: &str, config: StageConfig) -> Result<Box<dyn Processor>> {
        let processor_config = RuleConfig::from_stage_config(&config)?;
        processor_config.validate()?;

        // Create timing mixin from processor configuration
        let timing = TimingMixin::new(processor_config.timing.as_ref());

        Ok(Box::new(Self {
            name: name.to_string(),
            config: processor_config,
            timing,
        }))
    }

    fn evaluate_condition(&self, payload: &Value, condition: &Condition) -> bool {
        let field_value = match FieldUtils::extract_field_value(payload, &condition.field_path) {
            Some(value) => value,
            None => {
                debug!("Field '{}' not found in payload", condition.field_path);
                return false;
            }
        };

        // Parse the operation string to ConditionOperation enum
        let operation = match ConditionOperation::from_str(&condition.operation) {
            Some(op) => op,
            None => {
                warn!("Unknown condition operation: {}", condition.operation);
                return false;
            }
        };

        // Use ConditionEvaluator to evaluate the condition
        ConditionEvaluator::evaluate_condition(field_value, &operation, &condition.value)
    }

    fn execute_action(&self, payload: &mut Value, action: &Action) -> Result<()> {
        let result = match action {
            Action::SetField { field_path, value } => {
                debug!("Setting field '{}' to {:?}", field_path, value);
                FieldUtils::set_field_value(payload, field_path, value.clone())
            }
            Action::RemoveField { field_path } => {
                debug!("Removing field '{}'", field_path);
                FieldUtils::remove_field_value(payload, field_path)
            }
            Action::CopyField {
                source_field,
                target_field,
            } => {
                debug!("Copying field '{}' to '{}'", source_field, target_field);
                if let Some(source_value) = FieldUtils::extract_field_value(payload, source_field) {
                    FieldUtils::set_field_value(payload, target_field, source_value.clone())
                } else {
                    let err = anyhow!(
                        "Source field '{}' not found for copy operation",
                        source_field
                    );
                    return self.handle_action_error(err, action);
                }
            }
            Action::RenameField {
                old_field,
                new_field,
            } => {
                debug!("Renaming field '{}' to '{}'", old_field, new_field);
                if let Some(value) = FieldUtils::extract_field_value(payload, old_field) {
                    let value_clone = value.clone();
                    match FieldUtils::set_field_value(payload, new_field, value_clone) {
                        Ok(_) => FieldUtils::remove_field_value(payload, old_field),
                        Err(e) => Err(e),
                    }
                } else {
                    let err = anyhow!("Field '{}' not found for rename operation", old_field);
                    return self.handle_action_error(err, action);
                }
            }
            Action::ComputeField {
                field_path,
                expression,
            } => {
                debug!(
                    "Computing field '{}' with expression '{}'",
                    field_path, expression
                );
                match self.evaluate_expression(payload, expression) {
                    Ok(result) => FieldUtils::set_field_value(
                        payload,
                        field_path,
                        Value::Number(Number::from_f64(result).unwrap_or(Number::from(0))),
                    ),
                    Err(e) => {
                        return self.handle_action_error(e, action);
                    }
                }
            }
            Action::DropMessage => {
                debug!("Drop message action encountered");
                Ok(())
            }
            Action::PassThrough => {
                debug!("Pass through action - no modifications");
                Ok(())
            }
            Action::KeepOnlyFields { .. } => {
                debug!("KeepOnlyFields action - handled in execute_actions");
                Ok(())
            }
        };

        match result {
            Ok(_) => Ok(()),
            Err(e) => self.handle_action_error(e, action),
        }
    }

    fn handle_action_error(&self, error: anyhow::Error, action: &Action) -> Result<()> {
        match self.config.error_strategy {
            ErrorStrategy::Continue => {
                error!("Action {:?} failed: {} (continuing)", action, error);
                Ok(())
            }
            ErrorStrategy::Skip => {
                warn!("Skipping action {:?} due to error: {}", action, error);
                Ok(())
            }
            ErrorStrategy::Abort => {
                error!("Aborting message processing due to action error: {}", error);
                Err(error)
            }
            ErrorStrategy::UseDefault => {
                warn!(
                    "Action {:?} failed: {} (using default behavior)",
                    action, error
                );
                // |KB|Todo: For now, this is the same as Continue, but could 
                // be enhanced to provide default values for specific action types
                Ok(())
            }
        }
    }

    fn keep_only_fields(&self, payload: &mut Value, field_paths: &[String]) -> Result<()> {
        // Extract all the values we want to keep first
        let mut kept_values = HashMap::new();

        for field_path in field_paths {
            if let Some(value) = FieldUtils::extract_field_value(payload, field_path) {
                kept_values.insert(field_path.clone(), value.clone());
            } else {
                warn!("Field '{}' not found while keeping fields", field_path);
            }
        }

        // Clear the payload and rebuild it with only the kept fields
        *payload = Value::Object(serde_json::Map::new());

        // Set each kept field back into the payload
        for (field_path, value) in kept_values {
            FieldUtils::set_field_value(payload, &field_path, value)?;
        }

        debug!("Kept {} fields: {:?}", field_paths.len(), field_paths);
        Ok(())
    }

    fn build_expression_context(&self, payload: &Value) -> evalexpr::HashMapContext {
        use evalexpr::HashMapContext;

        let mut context = HashMapContext::new();
        self.add_payload_to_context(payload, "", &mut context);
        context
    }

    fn add_payload_to_context(
        &self,
        value: &Value,
        prefix: &str,
        context: &mut evalexpr::HashMapContext,
    ) {
        use evalexpr::{ContextWithMutableVariables, Value as EvalValue};

        match value {
            Value::Number(num) => {
                if let Some(float_val) = num.as_f64() {
                    let _ = context.set_value(prefix.to_string(), EvalValue::Float(float_val));
                    debug!("Added numeric field '{}' = {}", prefix, float_val);
                }
            }
            Value::Bool(b) => {
                let _ = context.set_value(prefix.to_string(), EvalValue::Boolean(*b));
                debug!("Added boolean field '{}' = {}", prefix, b);
            }
            Value::String(s) => {
                let _ = context.set_value(prefix.to_string(), EvalValue::String(s.clone()));
                debug!("Added string field '{}' = '{}'", prefix, s);
            }
            Value::Object(map) => {
                for (key, val) in map {
                    let field_path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    self.add_payload_to_context(val, &field_path, context);
                }
            }
            Value::Array(arr) => {
                for (index, val) in arr.iter().enumerate() {
                    let field_path = if prefix.is_empty() {
                        format!("[{}]", index)
                    } else {
                        format!("{}[{}]", prefix, index)
                    };
                    self.add_payload_to_context(val, &field_path, context);
                }
            }
            Value::Null => {
                // Skip null values
            }
        }
    }

    fn evaluate_expression(&self, payload: &Value, expression: &str) -> Result<f64> {
        debug!("Evaluating expression: '{}'", expression);

        // Build context with all payload fields
        let context = self.build_expression_context(payload);

        // Transform common math functions to their namespaced versions for compatibility
        let mut processed_expression = expression.to_string();

        let math_functions = [
            ("sqrt", "math::sqrt"),
            ("sin", "math::sin"),
            ("cos", "math::cos"),
            ("tan", "math::tan"),
            ("log", "math::log10"),
            ("ln", "math::ln"),
            ("abs", "math::abs"),
            ("floor", "math::floor"),
            ("ceil", "math::ceil"),
            ("exp", "math::exp"),
        ];

        for (func, namespaced_func) in &math_functions {
            let pattern = format!(r"\b{}\b", regex::escape(func));
            if let Ok(regex) = regex::Regex::new(&pattern) {
                processed_expression = regex
                    .replace_all(&processed_expression, *namespaced_func)
                    .to_string();
            }
        }

        debug!("Processed expression: '{}'", processed_expression);

        // Evaluate using context
        match evalexpr::eval_float_with_context(&processed_expression, &context) {
            Ok(result) => {
                debug!("Expression '{}' evaluated to: {}", expression, result);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to evaluate expression '{}': {}", expression, e);
                Err(anyhow!("Expression evaluation failed: {}", e))
            }
        }
    }

    fn process_message(&self, mut message: Message) -> Result<Option<Message>> {
        let mut should_drop = false;

        for rule in &self.config.rules {
            if self.evaluate_condition(&message.payload, &rule.condition) {
                debug!("Rule condition matched for message from {}", message.source);

                if let Err(e) = self.execute_actions(&mut message.payload, &rule.actions) {
                    error!("Failed to execute actions: {}", e);
                }

                // Check if any action was a drop message
                for action in &rule.actions {
                    if matches!(action, Action::DropMessage) {
                        should_drop = true;
                        break;
                    }
                }
            } else if !rule.else_actions.is_empty() {
                debug!(
                    "Rule condition not matched, executing else_actions for message from {}",
                    message.source
                );

                if let Err(e) = self.execute_actions(&mut message.payload, &rule.else_actions) {
                    error!("Failed to execute else_actions: {}", e);
                }

                // Check if any else_action was a drop message
                for action in &rule.else_actions {
                    if matches!(action, Action::DropMessage) {
                        should_drop = true;
                        break;
                    }
                }
            }

            if should_drop {
                break;
            }
        }

        if should_drop {
            debug!("Message dropped due to drop_message action");
            return Ok(None);
        }

        // Update the source to indicate transformation
        message.source = self.name.clone();
        Ok(Some(message))
    }

    fn execute_actions(&self, payload: &mut Value, actions: &[Action]) -> Result<()> {
        // Pre-computation phase: evaluate all compute_field expressions before any destructive operations
        let mut computed_values = HashMap::new();
        for action in actions {
            if action.needs_pre_computation() {
                if let Action::ComputeField {
                    field_path,
                    expression,
                } = action
                {
                    debug!(
                        "Pre-computing field '{}' with expression '{}'",
                        field_path, expression
                    );
                    match self.evaluate_expression(payload, expression) {
                        Ok(result) => {
                            computed_values.insert(field_path.clone(), result);
                            debug!("Pre-computed '{}' = {}", field_path, result);
                        }
                        Err(e) => {
                            error!(
                                "Failed to pre-compute field '{}' with expression '{}': {}",
                                field_path, expression, e
                            );
                            computed_values.insert(field_path.clone(), 0.0); // Fallback value
                        }
                    }
                }
            }
        }

        // Sort actions by priority: KeepOnlyFields first, then transform actions, then control actions
        let mut sorted_actions: Vec<_> = actions.iter().collect();
        sorted_actions.sort_by(|a, b| {
            a.priority()
                .partial_cmp(&b.priority())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Execute actions in priority order
        for action in sorted_actions {
            match action {
                Action::ComputeField { field_path, .. } => {
                    // Use pre-computed value instead of re-evaluating
                    if let Some(computed_value) = computed_values.get(field_path) {
                        debug!(
                            "Setting pre-computed field '{}' to {}",
                            field_path, computed_value
                        );
                        FieldUtils::set_field_value(
                            payload,
                            field_path,
                            Value::Number(
                                Number::from_f64(*computed_value).unwrap_or(Number::from(0)),
                            ),
                        )?;
                    }
                }
                Action::KeepOnlyFields { field_paths } => {
                    debug!(
                        "Applying destructive reset - keeping only fields: {:?}",
                        field_paths
                    );
                    self.keep_only_fields(payload, field_paths)?;
                }
                _ => {
                    // Handle all other actions normally
                    self.execute_action(payload, action)?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Processor for RuleProcessor {
    async fn init(&mut self) -> Result<()> {
        tracing::info!(
            "Rule processor '{}' initialised with timing semantics and {} rules",
            self.name,
            self.config.rules.len()
        );
        Ok(())
    }

    async fn process(&mut self, context: &mut ProcessingContext) -> Result<()> {
        // Process all input channels
        for (channel_name, input) in context.inputs.iter_mut() {
            select! {
                message = input.recv() => {
                    if let Some(message) = message {
                        match self.process_message(message) {
                            Ok(Some(transformed_message)) => {
                                if let Some(output_info) = &context.output {
                                    // Preserve timing information when forwarding
                                    let output_message = Message {
                                        source: transformed_message.source,
                                        topic: output_info.name.clone(),
                                        payload: transformed_message.payload,
                                        timestamp: transformed_message.timestamp,
                                        timing: transformed_message.timing,
                                    };

                                    // Update watermark using timing mixin
                                    let output_message = self.timing.update_message_watermark(output_message);

                                    if let Err(e) = output_info.channel.publish(output_message).await {
                                        tracing::warn!("Failed to publish transformed message: {:?}", e);
                                    } else {
                                        tracing::debug!(
                                            "Message from '{}' transformed and forwarded",
                                            channel_name
                                        );
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::debug!("Message from '{}' was dropped by rule processor", channel_name);
                            }
                            Err(e) => {
                                error!("Failed to transform message: {}", e);
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    // Timeout - no messages received, continue processing
                    break;
                }
            }
        }
        Ok(())
    }
}

impl WithTimingMixin for RuleProcessor {
    fn timing_mixin(&self) -> &TimingMixin {
        &self.timing
    }

    fn timing_mixin_mut(&mut self) -> &mut TimingMixin {
        &mut self.timing
    }
}
