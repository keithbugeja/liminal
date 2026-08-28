//! Resolved graph model for visualising Liminal pipeline configurations.
//!
//! Liminal configs connect stages through channel names rather than explicit
//! from/to node references. This module materialises those implicit references
//! into a graph shape that a GUI can render and validate.

use crate::config::types::{ChannelConfig, ChannelType, Config, StageConfig};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Input,
    PipelineStage,
    Output,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: GraphNodeKind,
    pub display_name: String,
    pub pipeline_name: Option<String>,
    pub processor_type: String,
    pub input_channels: Vec<String>,
    pub output_channel: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub channel_name: String,
    pub target_input_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GraphChannel {
    pub name: String,
    pub producer_node_ids: Vec<String>,
    pub consumer_node_ids: Vec<String>,
    pub channel_type: String,
    pub capacity: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiagnosticKind {
    DanglingInputChannel,
    DuplicateChannelProducer,
    OrphanProducedChannel,
    CycleDetected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GraphDiagnostic {
    pub kind: GraphDiagnosticKind,
    pub severity: GraphDiagnosticSeverity,
    pub message: String,
    pub channel_name: Option<String>,
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResolvedPipelineGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub channels: Vec<GraphChannel>,
    pub diagnostics: Vec<GraphDiagnostic>,
}

#[derive(Clone, Debug)]
struct Producer {
    node_id: String,
    channel_config: ChannelConfig,
}

impl ResolvedPipelineGraph {
    pub fn from_config(config: &Config) -> Self {
        let mut nodes = collect_nodes(config);
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        let mut producers = collect_producers(&nodes, config);
        for producer_set in producers.values_mut() {
            producer_set.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        }

        let mut consumer_map = collect_consumers(&nodes);
        for consumer_set in consumer_map.values_mut() {
            consumer_set.sort();
            consumer_set.dedup();
        }

        let mut diagnostics = Vec::new();
        diagnostics.extend(duplicate_producer_diagnostics(&producers));

        let mut edges = Vec::new();
        for node in &nodes {
            for (input_index, channel_name) in node.input_channels.iter().enumerate() {
                match producers.get(channel_name).map(Vec::as_slice) {
                    None | Some([]) => {
                        diagnostics.push(dangling_input_diagnostic(channel_name, &node.id))
                    }
                    Some([producer]) => edges.push(GraphEdge {
                        id: edge_id(&producer.node_id, &node.id, channel_name, input_index),
                        source_node_id: producer.node_id.clone(),
                        target_node_id: node.id.clone(),
                        channel_name: channel_name.clone(),
                        target_input_index: input_index,
                    }),
                    Some(_) => {
                        // The duplicate-producer diagnostic carries the channel-level error.
                        // Avoid inventing a misleading edge when the producer is ambiguous.
                    }
                }
            }
        }

        edges.sort_by(|a, b| a.id.cmp(&b.id));

        let mut channels = collect_channels(&producers, &consumer_map);
        channels.sort_by(|a, b| a.name.cmp(&b.name));

        diagnostics.extend(orphan_channel_diagnostics(&channels));
        diagnostics.extend(cycle_diagnostics(&nodes, &edges));
        diagnostics.sort_by(|a, b| {
            diagnostic_sort_key(a)
                .cmp(&diagnostic_sort_key(b))
                .then_with(|| a.message.cmp(&b.message))
        });

        Self {
            nodes,
            edges,
            channels,
            diagnostics,
        }
    }
}

fn collect_nodes(config: &Config) -> Vec<GraphNode> {
    let mut nodes = Vec::new();

    for (name, stage_config) in &config.inputs {
        nodes.push(graph_node(
            input_node_id(name),
            GraphNodeKind::Input,
            name,
            None,
            stage_config,
        ));
    }

    for (pipeline_name, pipeline_config) in &config.pipelines {
        for (stage_name, stage_config) in &pipeline_config.stages {
            nodes.push(graph_node(
                pipeline_stage_node_id(pipeline_name, stage_name),
                GraphNodeKind::PipelineStage,
                stage_name,
                Some(pipeline_name.clone()),
                stage_config,
            ));
        }
    }

    for (name, stage_config) in &config.outputs {
        nodes.push(graph_node(
            output_node_id(name),
            GraphNodeKind::Output,
            name,
            None,
            stage_config,
        ));
    }

    nodes
}

fn graph_node(
    id: String,
    kind: GraphNodeKind,
    display_name: &str,
    pipeline_name: Option<String>,
    stage_config: &StageConfig,
) -> GraphNode {
    GraphNode {
        id,
        kind,
        display_name: display_name.to_string(),
        pipeline_name,
        processor_type: stage_config.r#type.clone(),
        input_channels: stage_config.inputs.clone().unwrap_or_default(),
        output_channel: stage_config.output.clone(),
    }
}

fn collect_producers(nodes: &[GraphNode], config: &Config) -> HashMap<String, Vec<Producer>> {
    let configs_by_id = stage_configs_by_node_id(config);
    let mut producers: HashMap<String, Vec<Producer>> = HashMap::new();

    for node in nodes {
        if let Some(output_channel) = &node.output_channel {
            let channel_config = configs_by_id
                .get(&node.id)
                .and_then(|config| config.channel.clone())
                .unwrap_or_default();

            producers
                .entry(output_channel.clone())
                .or_default()
                .push(Producer {
                    node_id: node.id.clone(),
                    channel_config,
                });
        }
    }

    producers
}

fn stage_configs_by_node_id(config: &Config) -> HashMap<String, &StageConfig> {
    let mut configs = HashMap::new();

    for (name, stage_config) in &config.inputs {
        configs.insert(input_node_id(name), stage_config);
    }

    for (pipeline_name, pipeline_config) in &config.pipelines {
        for (stage_name, stage_config) in &pipeline_config.stages {
            configs.insert(
                pipeline_stage_node_id(pipeline_name, stage_name),
                stage_config,
            );
        }
    }

    for (name, stage_config) in &config.outputs {
        configs.insert(output_node_id(name), stage_config);
    }

    configs
}

fn collect_consumers(nodes: &[GraphNode]) -> HashMap<String, Vec<String>> {
    let mut consumers: HashMap<String, Vec<String>> = HashMap::new();

    for node in nodes {
        for channel_name in &node.input_channels {
            consumers
                .entry(channel_name.clone())
                .or_default()
                .push(node.id.clone());
        }
    }

    consumers
}

fn collect_channels(
    producers: &HashMap<String, Vec<Producer>>,
    consumers: &HashMap<String, Vec<String>>,
) -> Vec<GraphChannel> {
    let mut names: HashSet<String> = producers.keys().cloned().collect();
    names.extend(consumers.keys().cloned());

    names
        .into_iter()
        .map(|name| {
            let producer_node_ids = producers
                .get(&name)
                .map(|items| items.iter().map(|item| item.node_id.clone()).collect())
                .unwrap_or_default();

            let consumer_node_ids = consumers.get(&name).cloned().unwrap_or_default();
            let channel_config = producers
                .get(&name)
                .and_then(|items| items.first())
                .map(|producer| producer.channel_config.clone())
                .unwrap_or_default();

            GraphChannel {
                name,
                producer_node_ids,
                consumer_node_ids,
                channel_type: channel_type_name(&channel_config.r#type).to_string(),
                capacity: channel_config.capacity,
            }
        })
        .collect()
}

fn duplicate_producer_diagnostics(
    producers: &HashMap<String, Vec<Producer>>,
) -> Vec<GraphDiagnostic> {
    let mut diagnostics = Vec::new();

    for (channel_name, producer_set) in producers {
        if producer_set.len() > 1 {
            let node_ids: Vec<String> = producer_set
                .iter()
                .map(|producer| producer.node_id.clone())
                .collect();

            diagnostics.push(GraphDiagnostic {
                kind: GraphDiagnosticKind::DuplicateChannelProducer,
                severity: GraphDiagnosticSeverity::Error,
                message: format!(
                    "Channel '{}' is produced by multiple nodes: {}",
                    channel_name,
                    node_ids.join(", ")
                ),
                channel_name: Some(channel_name.clone()),
                node_ids,
            });
        }
    }

    diagnostics
}

fn dangling_input_diagnostic(channel_name: &str, node_id: &str) -> GraphDiagnostic {
    GraphDiagnostic {
        kind: GraphDiagnosticKind::DanglingInputChannel,
        severity: GraphDiagnosticSeverity::Error,
        message: format!(
            "Node '{}' consumes channel '{}' but nothing produces it",
            node_id, channel_name
        ),
        channel_name: Some(channel_name.to_string()),
        node_ids: vec![node_id.to_string()],
    }
}

fn orphan_channel_diagnostics(channels: &[GraphChannel]) -> Vec<GraphDiagnostic> {
    channels
        .iter()
        .filter(|channel| {
            channel.producer_node_ids.len() == 1 && channel.consumer_node_ids.is_empty()
        })
        .map(|channel| GraphDiagnostic {
            kind: GraphDiagnosticKind::OrphanProducedChannel,
            severity: GraphDiagnosticSeverity::Warning,
            message: format!(
                "Channel '{}' is produced by '{}' but has no consumers",
                channel.name, channel.producer_node_ids[0]
            ),
            channel_name: Some(channel.name.clone()),
            node_ids: channel.producer_node_ids.clone(),
        })
        .collect()
}

fn cycle_diagnostics(nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<GraphDiagnostic> {
    let transform_node_ids: HashSet<&str> = nodes
        .iter()
        .filter(|node| node.kind == GraphNodeKind::PipelineStage)
        .map(|node| node.id.as_str())
        .collect();

    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        if transform_node_ids.contains(edge.source_node_id.as_str())
            && transform_node_ids.contains(edge.target_node_id.as_str())
        {
            adjacency
                .entry(edge.source_node_id.as_str())
                .or_default()
                .push(edge.target_node_id.as_str());
        }
    }

    for targets in adjacency.values_mut() {
        targets.sort();
        targets.dedup();
    }

    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut active = HashSet::new();
    let mut stack = Vec::new();

    let mut node_ids: Vec<&str> = transform_node_ids.into_iter().collect();
    node_ids.sort();

    for node_id in node_ids {
        find_cycles(
            node_id,
            &adjacency,
            &mut visited,
            &mut active,
            &mut stack,
            &mut cycles,
        );
    }

    cycles
        .into_iter()
        .map(|cycle| GraphDiagnostic {
            kind: GraphDiagnosticKind::CycleDetected,
            severity: GraphDiagnosticSeverity::Error,
            message: format!(
                "Cycle detected between pipeline stages: {}",
                cycle.join(" -> ")
            ),
            channel_name: None,
            node_ids: cycle,
        })
        .collect()
}

fn find_cycles<'a>(
    node_id: &'a str,
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    active: &mut HashSet<&'a str>,
    stack: &mut Vec<&'a str>,
    cycles: &mut Vec<Vec<String>>,
) {
    if active.contains(node_id) {
        if let Some(start_index) = stack.iter().position(|id| *id == node_id) {
            let mut cycle: Vec<String> = stack[start_index..]
                .iter()
                .map(|id| (*id).to_string())
                .collect();
            cycle.push(node_id.to_string());
            cycles.push(cycle);
        }
        return;
    }

    if visited.contains(node_id) {
        return;
    }

    visited.insert(node_id);
    active.insert(node_id);
    stack.push(node_id);

    if let Some(targets) = adjacency.get(node_id) {
        for target in targets {
            find_cycles(target, adjacency, visited, active, stack, cycles);
        }
    }

    stack.pop();
    active.remove(node_id);
}

fn diagnostic_sort_key(diagnostic: &GraphDiagnostic) -> String {
    format!(
        "{:?}:{:?}:{}",
        diagnostic.severity,
        diagnostic.kind,
        diagnostic.channel_name.as_deref().unwrap_or("")
    )
}

fn channel_type_name(channel_type: &ChannelType) -> &'static str {
    match channel_type {
        ChannelType::Broadcast => "broadcast",
        ChannelType::Direct => "direct",
        ChannelType::Shared => "shared",
        ChannelType::Fanout => "fanout",
    }
}

fn input_node_id(name: &str) -> String {
    format!("input:{}", name)
}

fn pipeline_stage_node_id(pipeline_name: &str, stage_name: &str) -> String {
    format!("pipeline:{}.stage:{}", pipeline_name, stage_name)
}

fn output_node_id(name: &str) -> String {
    format!("output:{}", name)
}

fn edge_id(
    source_node_id: &str,
    target_node_id: &str,
    channel_name: &str,
    target_input_index: usize,
) -> String {
    format!(
        "{}--{}--{}--{}",
        source_node_id, channel_name, target_node_id, target_input_index
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::load_config_from_string;
    use crate::config::types::{PipelineConfig, StageConfig};
    use serde_json::json;
    use std::fs;

    #[test]
    fn resolves_simple_pipeline_graph() {
        let graph = graph_from_toml(
            r#"
            [inputs.sensor]
            type = "simulated"
            output = "raw"

            [pipelines.main]
            description = "main"

            [pipelines.main.stages.filter]
            type = "rule"
            inputs = ["raw"]
            output = "filtered"

            [outputs.console]
            type = "console"
            inputs = ["filtered"]
            "#,
        );

        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert!(graph.diagnostics.is_empty());
        assert_edge(&graph, "input:sensor", "pipeline:main.stage:filter", "raw");
        assert_edge(
            &graph,
            "pipeline:main.stage:filter",
            "output:console",
            "filtered",
        );
    }

    #[test]
    fn resolves_fan_out_from_one_channel() {
        let graph = graph_from_toml(
            r#"
            [inputs.sensor]
            type = "simulated"
            output = "raw"

            [pipelines.main]
            description = "main"

            [pipelines.main.stages.a]
            type = "rule"
            inputs = ["raw"]
            output = "a_out"

            [pipelines.main.stages.b]
            type = "rule"
            inputs = ["raw"]
            output = "b_out"
            "#,
        );

        let raw_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.channel_name == "raw")
            .collect();

        assert_eq!(raw_edges.len(), 2);
        assert!(
            graph
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.kind != GraphDiagnosticKind::DanglingInputChannel })
        );
    }

    #[test]
    fn reports_dangling_input_channel() {
        let graph = graph_from_toml(
            r#"
            [outputs.console]
            type = "console"
            inputs = ["missing"]
            "#,
        );

        assert_has_diagnostic(&graph, GraphDiagnosticKind::DanglingInputChannel, "missing");
    }

    #[test]
    fn reports_duplicate_channel_producers() {
        let graph = graph_from_toml(
            r#"
            [inputs.a]
            type = "simulated"
            output = "raw"

            [inputs.b]
            type = "simulated"
            output = "raw"

            [outputs.console]
            type = "console"
            inputs = ["raw"]
            "#,
        );

        assert_has_diagnostic(&graph, GraphDiagnosticKind::DuplicateChannelProducer, "raw");
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn reports_orphan_produced_channel() {
        let graph = graph_from_toml(
            r#"
            [inputs.sensor]
            type = "simulated"
            output = "raw"
            "#,
        );

        assert_has_diagnostic(&graph, GraphDiagnosticKind::OrphanProducedChannel, "raw");
    }

    #[test]
    fn reports_cycle_between_pipeline_stages() {
        let graph = graph_from_toml(
            r#"
            [pipelines.main]
            description = "main"

            [pipelines.main.stages.a]
            type = "rule"
            inputs = ["b_out"]
            output = "a_out"

            [pipelines.main.stages.b]
            type = "rule"
            inputs = ["a_out"]
            output = "b_out"
            "#,
        );

        assert!(
            graph
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == GraphDiagnosticKind::CycleDetected)
        );
    }

    #[test]
    fn resolves_cross_pipeline_channel_dependency() {
        let graph = graph_from_toml(
            r#"
            [inputs.sensor]
            type = "simulated"
            output = "raw"

            [pipelines.first]
            description = "first"

            [pipelines.first.stages.a]
            type = "rule"
            inputs = ["raw"]
            output = "a_out"

            [pipelines.second]
            description = "second"

            [pipelines.second.stages.b]
            type = "rule"
            inputs = ["a_out"]
            output = "b_out"
            "#,
        );

        assert_edge(
            &graph,
            "pipeline:first.stage:a",
            "pipeline:second.stage:b",
            "a_out",
        );
    }

    #[test]
    fn graph_json_export_is_serializable() {
        let graph = ResolvedPipelineGraph::from_config(&minimal_config());
        let value = serde_json::to_value(&graph).expect("graph serializes");

        assert_eq!(value["nodes"][0]["id"], json!("input:sensor"));
        assert!(value["edges"].is_array());
        assert!(value["channels"].is_array());
        assert!(value["diagnostics"].is_array());
    }

    #[test]
    fn resolves_existing_example_configs() {
        for entry in fs::read_dir("config/examples").expect("example config dir exists") {
            let entry = entry.expect("example config entry is readable");
            let path = entry.path();

            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }

            let content = fs::read_to_string(&path).expect("example config is readable");
            let config = load_config_from_string(&content)
                .unwrap_or_else(|error| panic!("{} should parse: {}", path.display(), error));
            let graph = ResolvedPipelineGraph::from_config(&config);

            assert!(
                graph
                    .diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.severity != GraphDiagnosticSeverity::Error),
                "{} has graph errors: {:#?}",
                path.display(),
                graph.diagnostics
            );
        }
    }

    fn graph_from_toml(toml: &str) -> ResolvedPipelineGraph {
        let config = load_config_from_string(toml).expect("test config parses");
        ResolvedPipelineGraph::from_config(&config)
    }

    fn minimal_config() -> Config {
        Config {
            inputs: HashMap::from([(
                "sensor".to_string(),
                StageConfig {
                    r#type: "simulated".to_string(),
                    inputs: None,
                    output: Some("raw".to_string()),
                    concurrency: None,
                    channel: None,
                    timing: None,
                    parameters: None,
                },
            )]),
            pipelines: HashMap::from([(
                "main".to_string(),
                PipelineConfig {
                    description: "main".to_string(),
                    stages: HashMap::new(),
                },
            )]),
            outputs: HashMap::new(),
        }
    }

    fn assert_edge(
        graph: &ResolvedPipelineGraph,
        source_node_id: &str,
        target_node_id: &str,
        channel_name: &str,
    ) {
        assert!(
            graph.edges.iter().any(|edge| {
                edge.source_node_id == source_node_id
                    && edge.target_node_id == target_node_id
                    && edge.channel_name == channel_name
            }),
            "expected edge {} --{}--> {} in {:#?}",
            source_node_id,
            channel_name,
            target_node_id,
            graph.edges
        );
    }

    fn assert_has_diagnostic(
        graph: &ResolvedPipelineGraph,
        kind: GraphDiagnosticKind,
        channel_name: &str,
    ) {
        assert!(
            graph.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == kind && diagnostic.channel_name.as_deref() == Some(channel_name)
            }),
            "expected diagnostic {:?} for channel {} in {:#?}",
            kind,
            channel_name,
            graph.diagnostics
        );
    }
}
