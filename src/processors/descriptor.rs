use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessorCategory {
    Input,
    Transform,
    Aggregator,
    Output,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    String,
    Integer,
    Number,
    Boolean,
    Enum,
    Array,
    Object,
    JsonValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchemaSpec {
    Object {
        fields: Vec<FieldSpec>,
    },
    Array {
        item: Box<SchemaSpec>,
    },
    TaggedUnion {
        tag: String,
        variants: Vec<TaggedVariantSpec>,
    },
    JsonValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TaggedVariantSpec {
    pub tag_value: String,
    pub label: String,
    pub fields: Vec<FieldSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FieldSpec {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    pub required: bool,
    pub default_value: Option<String>,
    pub options: Vec<String>,
    pub help: String,
    pub schema: Option<SchemaSpec>,
    pub renderer: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessorDescriptor {
    pub type_name: String,
    pub category: ProcessorCategory,
    pub display_name: String,
    pub description: String,
    pub fields: Vec<FieldSpec>,
}

pub fn processor_descriptors() -> Vec<ProcessorDescriptor> {
    let mut descriptors = vec![
        ProcessorDescriptor {
            type_name: "simulated".to_string(),
            category: ProcessorCategory::Input,
            display_name: "Simulated Signal".to_string(),
            description: "Generates synthetic numeric signal messages.".to_string(),
            fields: vec![
                field(
                    "field_out",
                    "Output field",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Payload field written by the generated value.",
                ),
                field(
                    "interval_ms",
                    "Interval",
                    FieldKind::Integer,
                    false,
                    Some("1000"),
                    &[],
                    "Delay between generated messages in milliseconds.",
                ),
                field(
                    "distribution",
                    "Distribution",
                    FieldKind::Enum,
                    false,
                    Some("uniform"),
                    &["uniform", "normal"],
                    "Random distribution used for generated values.",
                ),
                field(
                    "min_value",
                    "Minimum",
                    FieldKind::Number,
                    false,
                    Some("0.0"),
                    &[],
                    "Minimum generated numeric value.",
                ),
                field(
                    "max_value",
                    "Maximum",
                    FieldKind::Number,
                    false,
                    Some("100.0"),
                    &[],
                    "Maximum generated numeric value.",
                ),
            ],
        },
        ProcessorDescriptor {
            type_name: "mqtt_sub".to_string(),
            category: ProcessorCategory::Input,
            display_name: "MQTT Subscriber".to_string(),
            description: "Subscribes to MQTT topics and emits received messages.".to_string(),
            fields: vec![
                field(
                    "broker_url",
                    "Broker URL",
                    FieldKind::String,
                    false,
                    Some("mqtt://localhost:1883"),
                    &[],
                    "MQTT broker URL.",
                ),
                field(
                    "client_id",
                    "Client ID",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Optional MQTT client ID.",
                ),
                field(
                    "qos",
                    "QoS",
                    FieldKind::Integer,
                    false,
                    Some("0"),
                    &[],
                    "MQTT quality-of-service level, 0 through 2.",
                ),
                field(
                    "clean_session",
                    "Clean session",
                    FieldKind::Boolean,
                    false,
                    Some("true"),
                    &[],
                    "Whether the MQTT connection starts a clean session.",
                ),
                field(
                    "username",
                    "Username",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Optional broker username.",
                ),
                field(
                    "password",
                    "Password",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Optional broker password.",
                ),
                field_with_schema(
                    field(
                        "topics",
                        "Topics",
                        FieldKind::Array,
                        false,
                        Some("[\"#\"]"),
                        &[],
                        "MQTT topic filters to subscribe to.",
                    ),
                    SchemaSpec::Array {
                        item: Box::new(SchemaSpec::JsonValue),
                    },
                    Some("string_array"),
                ),
            ],
        },
        ProcessorDescriptor {
            type_name: "tcp_input".to_string(),
            category: ProcessorCategory::Input,
            display_name: "TCP Input".to_string(),
            description: "Receives length-prefixed JSON messages over TCP.".to_string(),
            fields: tcp_fields(),
        },
        ProcessorDescriptor {
            type_name: "rule".to_string(),
            category: ProcessorCategory::Transform,
            display_name: "Rule Processor".to_string(),
            description: "Applies conditional message transformation rules.".to_string(),
            fields: rule_fields(),
        },
        ProcessorDescriptor {
            type_name: "fusion".to_string(),
            category: ProcessorCategory::Aggregator,
            display_name: "Fusion".to_string(),
            description: "Combines messages from multiple inputs.".to_string(),
            fields: vec![],
        },
        ProcessorDescriptor {
            type_name: "console".to_string(),
            category: ProcessorCategory::Output,
            display_name: "Console Output".to_string(),
            description: "Consumes messages and writes them to the console/log.".to_string(),
            fields: vec![],
        },
        ProcessorDescriptor {
            type_name: "file".to_string(),
            category: ProcessorCategory::Output,
            display_name: "File Output".to_string(),
            description: "Writes consumed messages to a file.".to_string(),
            fields: vec![
                field(
                    "file_path",
                    "File path",
                    FieldKind::String,
                    true,
                    None,
                    &[],
                    "Destination file path.",
                ),
                field(
                    "format",
                    "Format",
                    FieldKind::Enum,
                    false,
                    Some("json"),
                    &["json", "csv", "text", "pretty"],
                    "Output serialization format.",
                ),
                field(
                    "append",
                    "Append",
                    FieldKind::Boolean,
                    false,
                    Some("true"),
                    &[],
                    "Append instead of overwriting.",
                ),
                field(
                    "create_dirs",
                    "Create directories",
                    FieldKind::Boolean,
                    false,
                    Some("true"),
                    &[],
                    "Create missing parent directories.",
                ),
                field(
                    "buffer_size",
                    "Buffer size",
                    FieldKind::Integer,
                    false,
                    Some("8192"),
                    &[],
                    "Write buffer size in bytes.",
                ),
                field(
                    "auto_flush",
                    "Auto flush",
                    FieldKind::Boolean,
                    false,
                    Some("false"),
                    &[],
                    "Flush after each message.",
                ),
            ],
        },
        ProcessorDescriptor {
            type_name: "mqtt_pub".to_string(),
            category: ProcessorCategory::Output,
            display_name: "MQTT Publisher".to_string(),
            description: "Publishes consumed messages to MQTT topics.".to_string(),
            fields: vec![
                field(
                    "broker_url",
                    "Broker URL",
                    FieldKind::String,
                    false,
                    Some("mqtt://localhost:1883"),
                    &[],
                    "MQTT broker URL.",
                ),
                field(
                    "client_id",
                    "Client ID",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Optional MQTT client ID.",
                ),
                field(
                    "qos",
                    "QoS",
                    FieldKind::Integer,
                    false,
                    Some("0"),
                    &[],
                    "MQTT quality-of-service level, 0 through 2.",
                ),
                field(
                    "clean_session",
                    "Clean session",
                    FieldKind::Boolean,
                    false,
                    Some("true"),
                    &[],
                    "Whether the MQTT connection starts a clean session.",
                ),
                field(
                    "username",
                    "Username",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Optional broker username.",
                ),
                field(
                    "password",
                    "Password",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Optional broker password.",
                ),
                field(
                    "topic_map",
                    "Topic map",
                    FieldKind::Object,
                    false,
                    Some("{}"),
                    &[],
                    "Map from input channel names to MQTT topics.",
                ),
                field(
                    "default_topic",
                    "Default topic",
                    FieldKind::String,
                    false,
                    None,
                    &[],
                    "Fallback MQTT topic for unmapped inputs.",
                ),
                field(
                    "retain",
                    "Retain",
                    FieldKind::Boolean,
                    false,
                    Some("false"),
                    &[],
                    "Publish retained MQTT messages.",
                ),
            ],
        },
        ProcessorDescriptor {
            type_name: "tcp_output".to_string(),
            category: ProcessorCategory::Output,
            display_name: "TCP Output".to_string(),
            description: "Sends length-prefixed JSON messages over TCP.".to_string(),
            fields: tcp_fields(),
        },
    ];

    descriptors.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    descriptors
}

fn field(
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
    required: bool,
    default_value: Option<&'static str>,
    options: &'static [&'static str],
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        key: key.to_string(),
        label: label.to_string(),
        kind,
        required,
        default_value: default_value.map(str::to_string),
        options: options.iter().map(|option| option.to_string()).collect(),
        help: help.to_string(),
        schema: None,
        renderer: None,
    }
}

fn field_with_schema(
    mut field: FieldSpec,
    schema: SchemaSpec,
    renderer: Option<&'static str>,
) -> FieldSpec {
    field.schema = Some(schema);
    field.renderer = renderer.map(str::to_string);
    field
}

fn rule_fields() -> Vec<FieldSpec> {
    vec![
        field_with_schema(
            field(
                "rules",
                "Rules",
                FieldKind::Array,
                true,
                None,
                &[],
                "Ordered rule list with conditions, actions, and else actions.",
            ),
            SchemaSpec::Array {
                item: Box::new(rule_schema()),
            },
            Some("rule_builder"),
        ),
        field(
            "error_strategy",
            "Error strategy",
            FieldKind::Enum,
            false,
            Some("continue"),
            &["continue", "skip", "abort", "use_default"],
            "Behavior when a rule action fails.",
        ),
    ]
}

fn rule_schema() -> SchemaSpec {
    SchemaSpec::Object {
        fields: vec![
            field_with_schema(
                field(
                    "condition",
                    "Condition",
                    FieldKind::Object,
                    true,
                    None,
                    &[],
                    "Message predicate that decides whether actions run.",
                ),
                condition_schema(),
                None,
            ),
            field_with_schema(
                field(
                    "actions",
                    "Actions",
                    FieldKind::Array,
                    true,
                    None,
                    &[],
                    "Actions executed when the condition matches.",
                ),
                SchemaSpec::Array {
                    item: Box::new(action_schema()),
                },
                None,
            ),
            field_with_schema(
                field(
                    "else_actions",
                    "Else actions",
                    FieldKind::Array,
                    false,
                    None,
                    &[],
                    "Actions executed when the condition does not match.",
                ),
                SchemaSpec::Array {
                    item: Box::new(action_schema()),
                },
                None,
            ),
        ],
    }
}

fn condition_schema() -> SchemaSpec {
    SchemaSpec::Object {
        fields: vec![
            field(
                "field_path",
                "Field path",
                FieldKind::String,
                true,
                None,
                &[],
                "Payload field path to evaluate.",
            ),
            field(
                "operation",
                "Operation",
                FieldKind::Enum,
                true,
                Some("equals"),
                &[
                    "equals",
                    "not_equals",
                    "startswith",
                    "endswith",
                    "contains",
                    ">",
                    ">=",
                    "<",
                    "<=",
                ],
                "Comparison operation.",
            ),
            field(
                "value",
                "Value",
                FieldKind::JsonValue,
                true,
                None,
                &[],
                "Expected JSON value.",
            ),
        ],
    }
}

fn action_schema() -> SchemaSpec {
    SchemaSpec::TaggedUnion {
        tag: "type".to_string(),
        variants: vec![
            action_variant(
                "set_field",
                "Set field",
                vec![
                    field(
                        "field_path",
                        "Field path",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "Payload field to set.",
                    ),
                    field(
                        "value",
                        "Value",
                        FieldKind::JsonValue,
                        true,
                        None,
                        &[],
                        "JSON value to assign.",
                    ),
                ],
            ),
            action_variant(
                "remove_field",
                "Remove field",
                vec![field(
                    "field_path",
                    "Field path",
                    FieldKind::String,
                    true,
                    None,
                    &[],
                    "Payload field to remove.",
                )],
            ),
            action_variant(
                "copy_field",
                "Copy field",
                vec![
                    field(
                        "source_field",
                        "Source field",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "Payload field to copy from.",
                    ),
                    field(
                        "target_field",
                        "Target field",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "Payload field to copy into.",
                    ),
                ],
            ),
            action_variant(
                "rename_field",
                "Rename field",
                vec![
                    field(
                        "old_field",
                        "Old field",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "Payload field to rename.",
                    ),
                    field(
                        "new_field",
                        "New field",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "New payload field path.",
                    ),
                ],
            ),
            action_variant(
                "compute_field",
                "Compute field",
                vec![
                    field(
                        "field_path",
                        "Field path",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "Payload field to write.",
                    ),
                    field(
                        "expression",
                        "Expression",
                        FieldKind::String,
                        true,
                        None,
                        &[],
                        "Expression evaluated against the payload.",
                    ),
                ],
            ),
            action_variant("drop_message", "Drop message", vec![]),
            action_variant("pass_through", "Pass through", vec![]),
            action_variant(
                "keep_only_fields",
                "Keep only fields",
                vec![field_with_schema(
                    field(
                        "field_paths",
                        "Field paths",
                        FieldKind::Array,
                        true,
                        None,
                        &[],
                        "Payload fields to retain.",
                    ),
                    SchemaSpec::Array {
                        item: Box::new(SchemaSpec::JsonValue),
                    },
                    None,
                )],
            ),
        ],
    }
}

fn action_variant(
    tag_value: &'static str,
    label: &'static str,
    fields: Vec<FieldSpec>,
) -> TaggedVariantSpec {
    TaggedVariantSpec {
        tag_value: tag_value.to_string(),
        label: label.to_string(),
        fields,
    }
}

fn tcp_fields() -> Vec<FieldSpec> {
    vec![
        field(
            "mode",
            "Mode",
            FieldKind::Enum,
            false,
            Some("client"),
            &["client", "server"],
            "TCP connection mode.",
        ),
        field(
            "host",
            "Host",
            FieldKind::String,
            false,
            Some("localhost"),
            &[],
            "TCP host or bind address.",
        ),
        field(
            "port",
            "Port",
            FieldKind::Integer,
            false,
            Some("8080"),
            &[],
            "TCP port.",
        ),
        field(
            "reconnect",
            "Reconnect",
            FieldKind::Boolean,
            false,
            Some("true"),
            &[],
            "Reconnect when the TCP connection drops.",
        ),
        field(
            "reconnect_interval_ms",
            "Reconnect interval",
            FieldKind::Integer,
            false,
            Some("5000"),
            &[],
            "Delay between reconnect attempts in milliseconds.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_cover_default_processors() {
        let names = processor_descriptors()
            .into_iter()
            .map(|descriptor| descriptor.type_name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "console",
                "file",
                "fusion",
                "mqtt_pub",
                "mqtt_sub",
                "rule",
                "simulated",
                "tcp_input",
                "tcp_output",
            ]
        );
    }

    #[test]
    fn rule_descriptor_marks_rules_as_required_array() {
        let descriptors = processor_descriptors();
        let rule = descriptors
            .iter()
            .find(|descriptor| descriptor.type_name == "rule")
            .expect("rule descriptor exists");
        let rules = rule
            .fields
            .iter()
            .find(|field| field.key == "rules")
            .expect("rules field exists");
        let error_strategy = rule
            .fields
            .iter()
            .find(|field| field.key == "error_strategy")
            .expect("error strategy field exists");

        assert_eq!(rules.kind, FieldKind::Array);
        assert!(rules.required);
        assert_eq!(error_strategy.kind, FieldKind::Enum);
        assert_eq!(
            error_strategy.options,
            vec!["continue", "skip", "abort", "use_default"]
        );
    }

    #[test]
    fn rule_descriptor_exposes_nested_rule_schema() {
        let descriptors = processor_descriptors();
        let rule = descriptors
            .iter()
            .find(|descriptor| descriptor.type_name == "rule")
            .expect("rule descriptor exists");
        let rules = rule
            .fields
            .iter()
            .find(|field| field.key == "rules")
            .expect("rules field exists");

        assert_eq!(rules.renderer.as_deref(), Some("rule_builder"));

        let Some(SchemaSpec::Array { item }) = &rules.schema else {
            panic!("rules has an array schema");
        };
        let SchemaSpec::Object { fields } = item.as_ref() else {
            panic!("rule items are objects");
        };

        assert!(fields.iter().any(|field| field.key == "condition"));
        assert!(fields.iter().any(|field| field.key == "actions"));
        assert!(fields.iter().any(|field| field.key == "else_actions"));

        let actions = fields
            .iter()
            .find(|field| field.key == "actions")
            .expect("actions field exists");
        let Some(SchemaSpec::Array { item }) = &actions.schema else {
            panic!("actions has an array schema");
        };
        let SchemaSpec::TaggedUnion { tag, variants } = item.as_ref() else {
            panic!("actions are a tagged union");
        };

        assert_eq!(tag, "type");
        assert!(
            variants
                .iter()
                .any(|variant| variant.tag_value == "set_field")
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.tag_value == "drop_message")
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.tag_value == "keep_only_fields")
        );
    }

    #[test]
    fn descriptors_are_sorted_by_processor_type() {
        let names = processor_descriptors()
            .into_iter()
            .map(|descriptor| descriptor.type_name)
            .collect::<Vec<_>>();
        let mut sorted_names = names.clone();

        sorted_names.sort();

        assert_eq!(names, sorted_names);
    }

    #[test]
    fn mqtt_sub_topics_use_string_array_renderer() {
        let descriptors = processor_descriptors();
        let mqtt_sub = descriptors
            .iter()
            .find(|descriptor| descriptor.type_name == "mqtt_sub")
            .expect("mqtt_sub descriptor exists");
        let topics = mqtt_sub
            .fields
            .iter()
            .find(|field| field.key == "topics")
            .expect("topics field exists");

        assert_eq!(topics.kind, FieldKind::Array);
        assert_eq!(topics.renderer.as_deref(), Some("string_array"));
        assert!(matches!(topics.schema, Some(SchemaSpec::Array { .. })));
    }
}
