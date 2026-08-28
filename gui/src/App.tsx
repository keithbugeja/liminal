import { invoke } from "@tauri-apps/api/core";
import {
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  BaseEdge,
  Controls,
  Edge,
  EdgeChange,
  EdgeLabelRenderer,
  EdgeProps,
  getBezierPath,
  Handle,
  MarkerType,
  MiniMap,
  Node,
  NodeChange,
  NodeProps,
  Position,
  ReactFlow,
  ReactFlowProvider,
} from "@xyflow/react";
import {
  AlertCircle,
  Boxes,
  CircleDot,
  FileJson,
  GitBranch,
  Info,
  Loader2,
  Network,
  RefreshCw,
  Search,
} from "lucide-react";
import { ReactNode, useCallback, useEffect, useMemo, useState } from "react";

type GraphNodeKind = "input" | "pipeline_stage" | "output";
type GraphLane = "inputs" | "pipeline_stages" | "outputs";
type DiagnosticSeverity = "warning" | "error";
type DiagnosticsFilter = "all" | "errors" | "warnings";

type ResolvedPipelineGraph = {
  schema_version: number;
  summary: GraphSummary;
  nodes: GraphNode[];
  edges: GraphEdge[];
  channels: GraphChannel[];
  diagnostics: GraphDiagnostic[];
};

type GraphSummary = {
  node_count: number;
  edge_count: number;
  channel_count: number;
  diagnostic_count: number;
  error_count: number;
  warning_count: number;
  has_errors: boolean;
};

type GraphNode = {
  id: string;
  kind: GraphNodeKind;
  lane: GraphLane;
  lane_index: number;
  display_name: string;
  config_path: string;
  pipeline_name: string | null;
  processor_type: string;
  input_channels: string[];
  output_channel: string | null;
  parameters: GraphParameter[];
  timing: GraphParameter[];
  concurrency_type: string;
};

type GraphParameter = {
  key: string;
  value: string;
  value_kind: string;
  editable: boolean;
};

type GraphEdge = {
  id: string;
  source_node_id: string;
  target_node_id: string;
  channel_name: string;
  target_input_index: number;
};

type GraphChannel = {
  name: string;
  producer_node_ids: string[];
  consumer_node_ids: string[];
  channel_type: string;
  capacity: number;
};

type GraphDiagnostic = {
  kind: string;
  severity: DiagnosticSeverity;
  message: string;
  channel_name: string | null;
  node_ids: string[];
};

type FlowNodeData = {
  graphNode: GraphNode;
  diagnostics: GraphDiagnostic[];
  channelDiagnosticCounts: Record<string, number>;
  selectedChannelName: string | null;
  onSelectChannel: (channelName: string) => void;
};

type FocusState = {
  activeNodeIds: Set<string>;
  activeEdgeIds: Set<string>;
};

const laneX: Record<GraphLane, number> = {
  inputs: 40,
  pipeline_stages: 460,
  outputs: 880,
};

const laneTitle: Record<GraphLane, string> = {
  inputs: "Inputs",
  pipeline_stages: "Pipeline Stages",
  outputs: "Outputs",
};

const initialConfigPath = "config/examples/config_rule_filter.toml";
const laneTop = 24;
const nodeGap = 96;
const nodeTypes = { liminalNode: LiminalNode };
const edgeTypes = { channelEdge: ChannelEdge };
const channelPalette = ["#67e5d8", "#8aa7ff", "#e2b24f", "#f28b82", "#b58cff", "#78d879"];

function App() {
  const [configPath, setConfigPath] = useState(initialConfigPath);
  const [exampleConfigs, setExampleConfigs] = useState<string[]>([]);
  const [graph, setGraph] = useState<ResolvedPipelineGraph | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedChannelName, setSelectedChannelName] = useState<string | null>(null);
  const [selectedDiagnosticKey, setSelectedDiagnosticKey] = useState<string | null>(null);
  const [diagnosticsFilter, setDiagnosticsFilter] = useState<DiagnosticsFilter>("all");
  const [filterText, setFilterText] = useState("");
  const [loadState, setLoadState] = useState<"idle" | "loading" | "error">("idle");
  const [saveState, setSaveState] = useState<"idle" | "saving" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const loadGraph = useCallback(async (path: string) => {
    setLoadState("loading");
    setError(null);

    try {
      const nextGraph = await invoke<ResolvedPipelineGraph>("load_graph", { path });
      setGraph(nextGraph);
      setSelectedNodeId(nextGraph.nodes[0]?.id ?? null);
      setSelectedChannelName(null);
      setSelectedDiagnosticKey(null);
      setLoadState("idle");
      setSaveState("idle");
    } catch (caught) {
      setGraph(null);
      setSelectedNodeId(null);
      setSelectedChannelName(null);
      setSelectedDiagnosticKey(null);
      setError(String(caught));
      setLoadState("error");
      setSaveState("idle");
    }
  }, []);

  useEffect(() => {
    invoke<string[]>("list_example_configs")
      .then(setExampleConfigs)
      .catch(() => setExampleConfigs([]));
    loadGraph(initialConfigPath);
  }, [loadGraph]);
  const selectNode = useCallback((id: string | null) => {
    setSelectedNodeId(id);
    setSelectedChannelName(null);
    setSelectedDiagnosticKey(null);
  }, []);
  const selectChannel = useCallback((channelName: string | null) => {
    setSelectedChannelName(channelName);
    if (channelName) {
      setSelectedNodeId(null);
    }
    setSelectedDiagnosticKey(null);
  }, []);
  const selectDiagnostic = useCallback((diagnostic: GraphDiagnostic, index: number) => {
    setSelectedDiagnosticKey(diagnosticKey(diagnostic, index));

    if (diagnostic.channel_name) {
      setSelectedChannelName(diagnostic.channel_name);
      setSelectedNodeId(null);
      return;
    }

    setSelectedNodeId(diagnostic.node_ids[0] ?? null);
    setSelectedChannelName(null);
  }, []);
  const selectDiagnosticByStep = useCallback(
    (step: 1 | -1) => {
      const diagnostics = graph?.diagnostics ?? [];
      if (diagnostics.length === 0) {
        return;
      }

      const currentIndex = diagnostics.findIndex(
        (diagnostic, index) => diagnosticKey(diagnostic, index) === selectedDiagnosticKey,
      );
      const nextIndex =
        currentIndex === -1
          ? step === 1
            ? 0
            : diagnostics.length - 1
          : (currentIndex + step + diagnostics.length) % diagnostics.length;

      selectDiagnostic(diagnostics[nextIndex], nextIndex);
    },
    [graph, selectDiagnostic, selectedDiagnosticKey],
  );
  const updateNodeParameter = useCallback(
    async (nodeId: string, parameterKey: string, value: string) => {
      setSaveState("saving");
      setError(null);

      try {
        const nextGraph = await invoke<ResolvedPipelineGraph>("update_node_parameter", {
          path: configPath,
          nodeId,
          parameterKey,
          value,
        });
        setGraph(nextGraph);
        setSelectedNodeId(nodeId);
        setSelectedChannelName(null);
        setSelectedDiagnosticKey(null);
        setSaveState("idle");
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [configPath],
  );
  const updateNodeField = useCallback(
    async (nodeId: string, fieldKey: string, value: string) => {
      setSaveState("saving");
      setError(null);

      try {
        const nextGraph = await invoke<ResolvedPipelineGraph>("update_node_field", {
          path: configPath,
          nodeId,
          fieldKey,
          value,
        });
        setGraph(nextGraph);
        setSelectedNodeId(nodeId);
        setSelectedChannelName(null);
        setSelectedDiagnosticKey(null);
        setSaveState("idle");
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [configPath],
  );

  return (
    <ReactFlowProvider>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand-block">
            <div className="brand-mark">
              <GitBranch size={21} />
            </div>
            <div>
              <h1>Liminal</h1>
              <p>Pipeline graph</p>
            </div>
          </div>

          <div className="path-row">
            <FileJson size={17} />
            <input
              value={configPath}
              onChange={(event) => setConfigPath(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  loadGraph(configPath);
                }
              }}
              aria-label="Config path"
            />
            <button
              className="icon-button"
              onClick={() => loadGraph(configPath)}
              aria-label="Reload graph"
              title="Reload graph"
            >
              {loadState === "loading" ? (
                <Loader2 className="spin" size={17} />
              ) : (
                <RefreshCw size={17} />
              )}
            </button>
          </div>

          <div className="example-list">
            {exampleConfigs.map((path) => (
              <button
                key={path}
                className={path === configPath ? "example active" : "example"}
                onClick={() => {
                  setConfigPath(path);
                  loadGraph(path);
                }}
              >
                {configFileName(path)}
              </button>
            ))}
          </div>

          <div className="search-row">
            <Search size={16} />
            <input
              value={filterText}
              onChange={(event) => setFilterText(event.target.value)}
              placeholder="Search nodes or channels"
              aria-label="Search nodes or channels"
            />
          </div>

          <SummaryPanel graph={graph} />
          <GraphStatusPanel graph={graph} loadState={loadState} saveState={saveState} error={error} />
          <DiagnosticsPanel
            graph={graph}
            filter={diagnosticsFilter}
            onFilterChange={setDiagnosticsFilter}
            selectedDiagnosticKey={selectedDiagnosticKey}
            onSelectDiagnostic={selectDiagnostic}
            onPreviousDiagnostic={() => selectDiagnosticByStep(-1)}
            onNextDiagnostic={() => selectDiagnosticByStep(1)}
          />
        </aside>

        <main className="workspace">
          <GraphCanvas
            graph={graph}
            selectedNodeId={selectedNodeId}
            selectedChannelName={selectedChannelName}
            selectedDiagnosticKey={selectedDiagnosticKey}
            filterText={filterText}
            onSelectNode={selectNode}
            onSelectChannel={selectChannel}
            error={error}
            loadState={loadState}
          />
        </main>

        <InspectorPanel
          graph={graph}
          configPath={configPath}
          selectedNodeId={selectedNodeId}
          selectedChannelName={selectedChannelName}
          selectedDiagnosticKey={selectedDiagnosticKey}
          onSelectNode={selectNode}
          onSelectChannel={selectChannel}
          onUpdateNodeParameter={updateNodeParameter}
          onUpdateNodeField={updateNodeField}
          saveState={saveState}
        />
      </div>
    </ReactFlowProvider>
  );
}

function SummaryPanel({ graph }: { graph: ResolvedPipelineGraph | null }) {
  const summary = graph?.summary;

  return (
    <section className="panel">
      <div className="panel-title">
        <Boxes size={16} />
        <span>Graph</span>
      </div>
      <div className="metric-grid">
        <Metric label="Nodes" value={summary?.node_count ?? 0} />
        <Metric label="Edges" value={summary?.edge_count ?? 0} />
        <Metric label="Channels" value={summary?.channel_count ?? 0} />
        <Metric label="Errors" value={summary?.error_count ?? 0} />
      </div>
    </section>
  );
}

function GraphStatusPanel({
  graph,
  loadState,
  saveState,
  error,
}: {
  graph: ResolvedPipelineGraph | null;
  loadState: "idle" | "loading" | "error";
  saveState: "idle" | "saving" | "error";
  error: string | null;
}) {
  const status = graphStatus(graph, loadState, saveState, error);

  return (
    <section className={`status-panel ${status.severity}`}>
      <div>
        <span>{status.label}</span>
        <p>{status.detail}</p>
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function DiagnosticsPanel({
  graph,
  filter,
  onFilterChange,
  selectedDiagnosticKey,
  onSelectDiagnostic,
  onPreviousDiagnostic,
  onNextDiagnostic,
}: {
  graph: ResolvedPipelineGraph | null;
  filter: DiagnosticsFilter;
  onFilterChange: (filter: DiagnosticsFilter) => void;
  selectedDiagnosticKey: string | null;
  onSelectDiagnostic: (diagnostic: GraphDiagnostic, index: number) => void;
  onPreviousDiagnostic: () => void;
  onNextDiagnostic: () => void;
}) {
  const diagnostics = graph?.diagnostics ?? [];
  const filteredDiagnostics = diagnostics
    .map((diagnostic, index) => ({ diagnostic, index }))
    .filter(({ diagnostic }) => diagnosticMatchesFilter(diagnostic, filter));
  const groupedDiagnostics = groupDiagnosticsBySeverity(filteredDiagnostics);

  return (
    <section className="panel diagnostics-panel">
      <div className="panel-title">
        <AlertCircle size={16} />
        <span>Diagnostics</span>
      </div>
      <div className="diagnostic-toolbar">
        <div className="segmented-control" aria-label="Diagnostics filter">
          <button className={filter === "all" ? "active" : ""} onClick={() => onFilterChange("all")}>
            All
          </button>
          <button className={filter === "errors" ? "active" : ""} onClick={() => onFilterChange("errors")}>
            Errors
          </button>
          <button className={filter === "warnings" ? "active" : ""} onClick={() => onFilterChange("warnings")}>
            Warnings
          </button>
        </div>
        <div className="diagnostic-nav">
          <button onClick={onPreviousDiagnostic} disabled={diagnostics.length === 0} aria-label="Previous diagnostic">
            Prev
          </button>
          <button onClick={onNextDiagnostic} disabled={diagnostics.length === 0} aria-label="Next diagnostic">
            Next
          </button>
        </div>
      </div>
      {diagnostics.length === 0 ? (
        <p className="empty-state">No graph diagnostics.</p>
      ) : filteredDiagnostics.length === 0 ? (
        <p className="empty-state">No diagnostics match this filter.</p>
      ) : (
        <div className="diagnostic-list">
          {groupedDiagnostics.map(({ severity, items }) => (
            <div className="diagnostic-group" key={severity}>
              <div className="diagnostic-group-title">
                <span>{severity}</span>
                <strong>{items.length}</strong>
              </div>
              {items.map(({ diagnostic, index }) => (
                <button
                  key={`${diagnostic.kind}-${diagnostic.channel_name}-${index}`}
                  className={[
                    "diagnostic",
                    diagnostic.severity,
                    diagnosticKey(diagnostic, index) === selectedDiagnosticKey ? "selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => onSelectDiagnostic(diagnostic, index)}
                >
                  <div className="diagnostic-meta">
                    <span>{diagnostic.kind}</span>
                    <code>{diagnostic.channel_name ?? diagnostic.node_ids[0] ?? "graph"}</code>
                  </div>
                  <p>{diagnostic.message}</p>
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function InspectorPanel({
  graph,
  configPath,
  selectedNodeId,
  selectedChannelName,
  selectedDiagnosticKey,
  onSelectNode,
  onSelectChannel,
  onUpdateNodeParameter,
  onUpdateNodeField,
  saveState,
}: {
  graph: ResolvedPipelineGraph | null;
  configPath: string;
  selectedNodeId: string | null;
  selectedChannelName: string | null;
  selectedDiagnosticKey: string | null;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
  onUpdateNodeParameter: (nodeId: string, parameterKey: string, value: string) => Promise<void>;
  onUpdateNodeField: (nodeId: string, fieldKey: string, value: string) => Promise<void>;
  saveState: "idle" | "saving" | "error";
}) {
  const selectedNode = graph?.nodes.find((node) => node.id === selectedNodeId) ?? null;
  const selectedChannel =
    graph?.channels.find((channel) => channel.name === selectedChannelName) ?? null;
  const nodeDiagnostics = selectedNode
    ? graph?.diagnostics.filter((diagnostic) => diagnostic.node_ids.includes(selectedNode.id)) ?? []
    : [];
  const channelDiagnostics = selectedChannelName
    ? graph?.diagnostics.filter((diagnostic) => diagnostic.channel_name === selectedChannelName) ?? []
    : [];
  const selectedDiagnostic =
    graph?.diagnostics.find((diagnostic, index) => diagnosticKey(diagnostic, index) === selectedDiagnosticKey) ??
    null;
  const selectNode = useCallback(
    (id: string | null) => {
      onSelectNode(id);
      onSelectChannel(null);
    },
    [onSelectChannel, onSelectNode],
  );
  const selectChannel = useCallback(
    (channelName: string | null) => {
      onSelectChannel(channelName);
      if (channelName) {
        onSelectNode(null);
      }
    },
    [onSelectChannel, onSelectNode],
  );

  return (
    <aside className="inspector">
      <div className="inspector-header">
        <Info size={17} />
        <span>Inspector</span>
      </div>

      {!graph ? (
        <p className="empty-state">No graph loaded.</p>
      ) : selectedChannel ? (
        <ChannelInspector
          channel={selectedChannel}
          graph={graph}
          diagnostics={channelDiagnostics}
          selectedDiagnostic={selectedDiagnostic}
          onSelectNode={selectNode}
        />
      ) : selectedChannelName ? (
        <MissingChannelInspector
          channelName={selectedChannelName}
          diagnostics={channelDiagnostics}
          selectedDiagnostic={selectedDiagnostic}
        />
      ) : selectedNode ? (
        <NodeInspector
          node={selectedNode}
          configPath={configPath}
          graph={graph}
          diagnostics={nodeDiagnostics}
          selectedDiagnostic={selectedDiagnostic}
          onSelectChannel={selectChannel}
          onUpdateParameter={onUpdateNodeParameter}
          onUpdateField={onUpdateNodeField}
          saveState={saveState}
        />
      ) : (
        <div className="inspector-empty">
          <Network size={24} />
          <p>Select a node, channel, or edge.</p>
        </div>
      )}
    </aside>
  );
}

function NodeInspector({
  node,
  configPath,
  graph,
  diagnostics,
  selectedDiagnostic,
  onSelectChannel,
  onUpdateParameter,
  onUpdateField,
  saveState,
}: {
  node: GraphNode;
  configPath: string;
  graph: ResolvedPipelineGraph;
  diagnostics: GraphDiagnostic[];
  selectedDiagnostic: GraphDiagnostic | null;
  onSelectChannel: (channelName: string | null) => void;
  onUpdateParameter: (nodeId: string, parameterKey: string, value: string) => Promise<void>;
  onUpdateField: (nodeId: string, fieldKey: string, value: string) => Promise<void>;
  saveState: "idle" | "saving" | "error";
}) {
  const incomingEdges = graph.edges.filter((edge) => edge.target_node_id === node.id);
  const outgoingEdges = graph.edges.filter((edge) => edge.source_node_id === node.id);
  const outputChannel = node.output_channel
    ? graph.channels.find((channel) => channel.name === node.output_channel) ?? null
    : null;

  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow={node.processor_type} title={node.display_name} />
      {selectedDiagnostic && <SelectedDiagnostic diagnostic={selectedDiagnostic} />}
      <InspectorSection title="Overview" defaultOpen>
        <KeyValue label="Kind" value={node.kind} />
        <KeyValue label="Config path" value={node.config_path} />
        <KeyValue label="File" value={configPath} />
        {node.pipeline_name && <KeyValue label="Pipeline" value={node.pipeline_name} />}
        <KeyValue label="Node ID" value={node.id} />
        <KeyValue label="Incoming" value={String(incomingEdges.length)} />
        <KeyValue label="Outgoing" value={String(outgoingEdges.length)} />
      </InspectorSection>

      <InspectorSection title="Channels" defaultOpen>
        <div className="inspector-subsection">
          <h4>Inputs</h4>
          {node.input_channels.length === 0 ? (
            <p className="empty-state">No input channels.</p>
          ) : (
            <ChannelButtonList channels={node.input_channels} onSelectChannel={onSelectChannel} />
          )}
        </div>
        <div className="inspector-subsection">
          <h4>Output</h4>
          {node.output_channel ? (
            <ChannelButtonList channels={[node.output_channel]} onSelectChannel={onSelectChannel} />
          ) : (
            <p className="empty-state">No output channel.</p>
          )}
        </div>
      </InspectorSection>

      <InspectorSection title="Stage Fields">
        <div className="parameter-list">
          {node.output_channel ? (
            <>
              <EditableFieldRow
                label="output"
                value={node.output_channel}
                valueKind="string"
                saveState={saveState}
                onSave={(value) => onUpdateField(node.id, "output", value)}
              />
              <EditableFieldRow
                label="channel.type"
                value={outputChannel?.channel_type ?? "broadcast"}
                valueKind="enum"
                options={["broadcast", "direct", "shared", "fanout"]}
                saveState={saveState}
                onSave={(value) => onUpdateField(node.id, "channel.type", value)}
              />
              <EditableFieldRow
                label="channel.capacity"
                value={String(outputChannel?.capacity ?? 128)}
                valueKind="number"
                saveState={saveState}
                onSave={(value) => onUpdateField(node.id, "channel.capacity", value)}
              />
            </>
          ) : (
            <p className="empty-state">No output or channel fields on this node.</p>
          )}
          <EditableFieldRow
            label="concurrency.type"
            value={node.concurrency_type}
            valueKind="enum"
            options={["thread", "pipeline", "owner"]}
            saveState={saveState}
            onSave={(value) => onUpdateField(node.id, "concurrency.type", value)}
          />
        </div>
      </InspectorSection>

      <InspectorSection title="Timing" badge={node.timing.length > 0 ? String(node.timing.length) : undefined}>
        <div className="parameter-list">
          <EditableFieldRow
            label="timing.event_time_field"
            value={timingValue(node, "event_time_field", "")}
            valueKind="string"
            saveState={saveState}
            onSave={(value) => onUpdateField(node.id, "timing.event_time_field", value)}
          />
          <EditableFieldRow
            label="timing.max_lateness_ms"
            value={timingValue(node, "max_lateness_ms", "30000")}
            valueKind="number"
            saveState={saveState}
            onSave={(value) => onUpdateField(node.id, "timing.max_lateness_ms", value)}
          />
          <EditableFieldRow
            label="timing.processing_timeout_ms"
            value={timingValue(node, "processing_timeout_ms", "")}
            valueKind="number"
            saveState={saveState}
            onSave={(value) => onUpdateField(node.id, "timing.processing_timeout_ms", value)}
          />
          <EditableFieldRow
            label="timing.jitter_bounds_ms"
            value={timingValue(node, "jitter_bounds_ms", "")}
            valueKind="number"
            saveState={saveState}
            onSave={(value) => onUpdateField(node.id, "timing.jitter_bounds_ms", value)}
          />
          <EditableFieldRow
            label="timing.metrics_enabled"
            value={timingValue(node, "metrics_enabled", "true")}
            valueKind="boolean"
            saveState={saveState}
            onSave={(value) => onUpdateField(node.id, "timing.metrics_enabled", value)}
          />
          {node.timing.some((field) => field.key === "watermark_strategy") && (
            <div className="parameter-row read-only">
              <div className="parameter-label">
                <strong>timing.watermark_strategy</strong>
                <span>object</span>
              </div>
              <pre>configured</pre>
            </div>
          )}
        </div>
      </InspectorSection>

      <InspectorSection title="Parameters" badge={String(node.parameters.length)}>
        {node.parameters.length === 0 ? (
          <p className="empty-state">No processor parameters.</p>
        ) : (
          <div className="parameter-list">
            {node.parameters.map((parameter) => (
              <ParameterRow
                key={parameter.key}
                nodeId={node.id}
                parameter={parameter}
                saveState={saveState}
                onUpdateParameter={onUpdateParameter}
              />
            ))}
          </div>
        )}
      </InspectorSection>

      <DiagnosticsList diagnostics={diagnostics} selectedDiagnostic={selectedDiagnostic} defaultOpen={Boolean(selectedDiagnostic)} />
    </div>
  );
}

function ParameterRow({
  nodeId,
  parameter,
  saveState,
  onUpdateParameter,
}: {
  nodeId: string;
  parameter: GraphParameter;
  saveState: "idle" | "saving" | "error";
  onUpdateParameter: (nodeId: string, parameterKey: string, value: string) => Promise<void>;
}) {
  const [draftValue, setDraftValue] = useState(parameter.value);
  const isDirty = draftValue !== parameter.value;

  useEffect(() => {
    setDraftValue(parameter.value);
  }, [parameter.value]);

  if (!parameter.editable) {
    return (
      <div className="parameter-row read-only">
        <div className="parameter-label">
          <strong>{parameter.key}</strong>
          <span>{parameter.value_kind}</span>
        </div>
        <pre>{parameter.value}</pre>
      </div>
    );
  }

  return (
    <div className="parameter-row">
      <div className="parameter-label">
        <strong>{parameter.key}</strong>
        <span>{parameter.value_kind}</span>
      </div>
      {parameter.value_kind === "boolean" ? (
        <label className="parameter-toggle">
          <input
            type="checkbox"
            checked={draftValue === "true"}
            onChange={(event) => setDraftValue(String(event.target.checked))}
          />
          <span>{draftValue}</span>
        </label>
      ) : (
        <input
          value={draftValue}
          type={parameter.value_kind === "number" ? "number" : "text"}
          onChange={(event) => setDraftValue(event.target.value)}
        />
      )}
      <button
        disabled={!isDirty || saveState === "saving"}
        onClick={() => onUpdateParameter(nodeId, parameter.key, draftValue)}
      >
        {saveState === "saving" ? "Saving" : "Save"}
      </button>
    </div>
  );
}

function EditableFieldRow({
  label,
  value,
  valueKind,
  options,
  saveState,
  onSave,
}: {
  label: string;
  value: string;
  valueKind: "string" | "number" | "enum" | "boolean";
  options?: string[];
  saveState: "idle" | "saving" | "error";
  onSave: (value: string) => Promise<void>;
}) {
  const [draftValue, setDraftValue] = useState(value);
  const isDirty = draftValue !== value;

  useEffect(() => {
    setDraftValue(value);
  }, [value]);

  return (
    <div className="parameter-row">
      <div className="parameter-label">
        <strong>{label}</strong>
        <span>{valueKind}</span>
      </div>
      {valueKind === "enum" ? (
        <select value={draftValue} onChange={(event) => setDraftValue(event.target.value)}>
          {(options ?? []).map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      ) : valueKind === "boolean" ? (
        <label className="parameter-toggle">
          <input
            type="checkbox"
            checked={draftValue === "true"}
            onChange={(event) => setDraftValue(String(event.target.checked))}
          />
          <span>{draftValue}</span>
        </label>
      ) : (
        <input
          value={draftValue}
          type={valueKind === "number" ? "number" : "text"}
          onChange={(event) => setDraftValue(event.target.value)}
        />
      )}
      <button disabled={!isDirty || saveState === "saving"} onClick={() => onSave(draftValue)}>
        {saveState === "saving" ? "Saving" : "Save"}
      </button>
    </div>
  );
}

function ChannelInspector({
  channel,
  graph,
  diagnostics,
  selectedDiagnostic,
  onSelectNode,
}: {
  channel: GraphChannel;
  graph: ResolvedPipelineGraph;
  diagnostics: GraphDiagnostic[];
  selectedDiagnostic: GraphDiagnostic | null;
  onSelectNode: (id: string | null) => void;
}) {
  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow="Channel" title={channel.name} />
      {selectedDiagnostic && <SelectedDiagnostic diagnostic={selectedDiagnostic} />}
      <KeyValue label="Type" value={channel.channel_type} />
      <KeyValue label="Capacity" value={String(channel.capacity)} />

      <InspectorSection title="Producers" badge={String(channel.producer_node_ids.length)} defaultOpen>
        <NodeButtonList nodeIds={channel.producer_node_ids} graph={graph} onSelectNode={onSelectNode} />
      </InspectorSection>

      <InspectorSection title="Consumers" badge={String(channel.consumer_node_ids.length)} defaultOpen>
        <NodeButtonList nodeIds={channel.consumer_node_ids} graph={graph} onSelectNode={onSelectNode} />
      </InspectorSection>

      <DiagnosticsList diagnostics={diagnostics} selectedDiagnostic={selectedDiagnostic} defaultOpen={Boolean(selectedDiagnostic)} />
    </div>
  );
}

function MissingChannelInspector({
  channelName,
  diagnostics,
  selectedDiagnostic,
}: {
  channelName: string;
  diagnostics: GraphDiagnostic[];
  selectedDiagnostic: GraphDiagnostic | null;
}) {
  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow="Unresolved Channel" title={channelName} />
      {selectedDiagnostic && <SelectedDiagnostic diagnostic={selectedDiagnostic} />}
      <DiagnosticsList diagnostics={diagnostics} selectedDiagnostic={selectedDiagnostic} defaultOpen />
    </div>
  );
}

function SelectedDiagnostic({ diagnostic }: { diagnostic: GraphDiagnostic }) {
  return (
    <div className={`selected-diagnostic ${diagnostic.severity}`}>
      <span>{diagnostic.severity}</span>
      <p>{diagnostic.message}</p>
    </div>
  );
}

function InspectorTitle({ eyebrow, title }: { eyebrow: string; title: string }) {
  return (
    <div className="inspector-title">
      <span>{eyebrow}</span>
      <h2>{title}</h2>
    </div>
  );
}

function InspectorSection({
  title,
  children,
  badge,
  defaultOpen = false,
}: {
  title: string;
  children: ReactNode;
  badge?: string;
  defaultOpen?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <details
      className="inspector-section"
      open={isOpen}
      onToggle={(event) => setIsOpen(event.currentTarget.open)}
    >
      <summary>
        <h3>{title}</h3>
        {badge !== undefined && <span className="section-badge">{badge}</span>}
      </summary>
      <div className="inspector-section-body">{children}</div>
    </details>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="key-value">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ChannelButtonList({
  channels,
  onSelectChannel,
}: {
  channels: string[];
  onSelectChannel: (channelName: string | null) => void;
}) {
  return (
    <div className="inspector-list">
      {channels.map((channel) => (
        <button key={channel} onClick={() => onSelectChannel(channel)}>
          {channel}
        </button>
      ))}
    </div>
  );
}

function NodeButtonList({
  nodeIds,
  graph,
  onSelectNode,
}: {
  nodeIds: string[];
  graph: ResolvedPipelineGraph;
  onSelectNode: (id: string | null) => void;
}) {
  if (nodeIds.length === 0) {
    return <p className="empty-state">None.</p>;
  }

  return (
    <div className="inspector-list">
      {nodeIds.map((nodeId) => {
        const node = graph.nodes.find((candidate) => candidate.id === nodeId);
        return (
          <button key={nodeId} onClick={() => onSelectNode(nodeId)}>
            {node?.display_name ?? nodeId}
          </button>
        );
      })}
    </div>
  );
}

function DiagnosticsList({
  diagnostics,
  selectedDiagnostic,
  defaultOpen = false,
}: {
  diagnostics: GraphDiagnostic[];
  selectedDiagnostic: GraphDiagnostic | null;
  defaultOpen?: boolean;
}) {
  return (
    <InspectorSection title="Diagnostics" badge={String(diagnostics.length)} defaultOpen={defaultOpen || diagnostics.length > 0}>
      {diagnostics.length === 0 ? (
        <p className="empty-state">No diagnostics.</p>
      ) : (
        <div className="inspector-diagnostics">
          {diagnostics.map((diagnostic, index) => (
            <div
              key={`${diagnostic.kind}-${diagnostic.channel_name}-${index}`}
              className={[
                "inspector-diagnostic",
                diagnostic.severity,
                selectedDiagnostic === diagnostic ? "selected" : "",
              ]
                .filter(Boolean)
                .join(" ")}
            >
              <span>{diagnostic.severity}</span>
              <p>{diagnostic.message}</p>
            </div>
          ))}
        </div>
      )}
    </InspectorSection>
  );
}

function GraphCanvas({
  graph,
  selectedNodeId,
  selectedChannelName,
  selectedDiagnosticKey,
  filterText,
  onSelectNode,
  onSelectChannel,
  error,
  loadState,
}: {
  graph: ResolvedPipelineGraph | null;
  selectedNodeId: string | null;
  selectedChannelName: string | null;
  selectedDiagnosticKey: string | null;
  filterText: string;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
  error: string | null;
  loadState: "idle" | "loading" | "error";
}) {
  const diagnosticsByNode = useMemo(() => diagnosticsByNodeId(graph), [graph]);
  const diagnosticsByChannel = useMemo(() => diagnosticCountByChannelName(graph), [graph]);
  const focusState = useMemo(
    () => graphFocusState(graph, selectedNodeId, selectedChannelName, selectedDiagnosticKey),
    [graph, selectedChannelName, selectedDiagnosticKey, selectedNodeId],
  );
  const channelByName = useMemo(() => {
    const map = new Map<string, GraphChannel>();
    graph?.channels.forEach((channel) => map.set(channel.name, channel));
    return map;
  }, [graph]);

  const flowNodes = useMemo<Node<FlowNodeData>[]>(() => {
    const query = filterText.trim().toLowerCase();
    const positions = layoutNodePositions(graph?.nodes ?? []);

    return (graph?.nodes ?? []).map((node) => {
      const channels = [...node.input_channels, node.output_channel ?? ""]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();
      const matches =
        !query ||
        node.display_name.toLowerCase().includes(query) ||
        node.processor_type.toLowerCase().includes(query) ||
        channels.includes(query);
      const isFocusMuted = Boolean(focusState) && !focusState?.activeNodeIds.has(node.id);
      const channelDiagnosticCounts = [...node.input_channels, node.output_channel ?? ""]
        .filter(Boolean)
        .reduce<Record<string, number>>((counts, channelName) => {
          counts[channelName] = diagnosticsByChannel.get(channelName) ?? 0;
          return counts;
        }, {});

      return {
        id: node.id,
        type: "liminalNode",
        position: positions.get(node.id) ?? { x: laneX[node.lane], y: laneTop },
        draggable: true,
        sourcePosition: Position.Right,
        targetPosition: Position.Left,
        selected: node.id === selectedNodeId,
        data: {
          graphNode: node,
          diagnostics: diagnosticsByNode.get(node.id) ?? [],
          channelDiagnosticCounts,
          selectedChannelName,
          onSelectChannel,
        },
        className: [
          matches ? "" : "dimmed-node",
          isFocusMuted ? "focus-muted" : "",
          focusState?.activeNodeIds.has(node.id) ? "focus-active" : "",
        ]
          .filter(Boolean)
          .join(" "),
      };
    });
  }, [
    diagnosticsByChannel,
    diagnosticsByNode,
    filterText,
    focusState,
    graph,
    onSelectChannel,
    selectedChannelName,
    selectedNodeId,
  ]);

  const flowEdges = useMemo<Edge[]>(() => {
    return (graph?.edges ?? []).map((edge, index) => {
      const channel = channelByName.get(edge.channel_name);
      const color = colorForChannel(edge.channel_name);
      const laneOffset = ((index % 5) - 2) * 14;
      const channelDiagnostics =
        graph?.diagnostics.filter((diagnostic) => diagnostic.channel_name === edge.channel_name) ?? [];
      const pathState = focusState
        ? focusState.activeEdgeIds.has(edge.id)
          ? "active"
          : "muted"
        : null;

      return {
        id: edge.id,
        source: edge.source_node_id,
        target: edge.target_node_id,
        sourceHandle: "output-channel",
        targetHandle: `input-channel-${edge.target_input_index}`,
        type: "channelEdge",
        markerEnd: {
          type: MarkerType.ArrowClosed,
          color,
          width: 16,
          height: 16,
        },
        label: edge.channel_name,
        data: {
          channelName: edge.channel_name,
          color,
          laneOffset,
          pathState,
          severity: severityForDiagnostics(channelDiagnostics),
        },
        animated: channel?.channel_type === "broadcast" || channel?.channel_type === "fanout",
        style: { stroke: color, strokeWidth: 2.1 },
      };
    });
  }, [channelByName, focusState, graph]);

  const [nodes, setNodes] = useState<Node<FlowNodeData>[]>(flowNodes);
  const [edges, setEdges] = useState<Edge[]>(flowEdges);
  const onNodesChange = useCallback(
    (changes: NodeChange<Node<FlowNodeData>>[]) => {
      setNodes((currentNodes) => applyNodeChanges<Node<FlowNodeData>>(changes, currentNodes));
    },
    [],
  );
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      setEdges((currentEdges) => applyEdgeChanges(changes, currentEdges));
    },
    [],
  );

  useEffect(() => {
    setNodes((currentNodes) =>
      flowNodes.map((flowNode) => {
        const currentNode = currentNodes.find((node) => node.id === flowNode.id);
        return currentNode ? { ...flowNode, position: currentNode.position } : flowNode;
      }),
    );
  }, [flowNodes, setNodes]);

  useEffect(() => {
    setEdges(flowEdges);
  }, [flowEdges, setEdges]);

  if (loadState === "error") {
    return (
      <div className="center-message error-message">
        <AlertCircle size={28} />
        <p>{error}</p>
      </div>
    );
  }

  if (!graph) {
    return (
      <div className="center-message">
        <Loader2 className="spin" size={28} />
      </div>
    );
  }

  return (
    <div className="canvas-shell">
      <LaneLabels graph={graph} />
      <div className="flow-area">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onNodeDragStop={(_, node) => setNodes((currentNodes) => mergeDraggedNode(currentNodes, node))}
          onNodeClick={(_, node) => {
            onSelectNode(node.id);
            onSelectChannel(null);
          }}
          onEdgeClick={(_, edge) => {
            const channelName = (edge.data as ChannelEdgeData | undefined)?.channelName;
            if (channelName) {
              onSelectChannel(channelName);
              onSelectNode(null);
            }
          }}
          onPaneClick={() => {
            onSelectNode(null);
            onSelectChannel(null);
          }}
          nodesDraggable
          nodesConnectable={false}
          elementsSelectable
          fitView
          fitViewOptions={{ padding: 0.18 }}
        >
          <Background color="#243235" gap={24} size={1} />
          <MiniMap
            pannable
            zoomable
            nodeColor={(node) => {
              const graphNode = (node.data as FlowNodeData).graphNode;
              if (graphNode.kind === "input") return "#4db7a7";
              if (graphNode.kind === "output") return "#d7a84d";
              return "#7d9cff";
            }}
          />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
    </div>
  );
}

function LiminalNode({ data, selected }: NodeProps<Node<FlowNodeData>>) {
  const { graphNode, diagnostics, channelDiagnosticCounts, selectedChannelName, onSelectChannel } = data;
  const hasError = diagnostics.some((diagnostic) => diagnostic.severity === "error");
  const hasWarning = diagnostics.some((diagnostic) => diagnostic.severity === "warning");
  const diagnosticSeverity = severityForDiagnostics(diagnostics);
  const className = [
    "liminal-node",
    graphNode.kind,
    selected ? "selected" : "",
    hasError ? "has-error" : "",
    hasWarning ? "has-warning" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const outputChannel = graphNode.output_channel;

  return (
    <div className={className}>
      <div className="node-header">
        <CircleDot size={14} />
        <span>{graphNode.processor_type}</span>
        {diagnosticSeverity && <DiagnosticBadge count={diagnostics.length} severity={diagnosticSeverity} />}
      </div>
      <strong>{graphNode.display_name}</strong>
      <p>{graphNode.config_path}</p>
      <div className="node-channels">
        {graphNode.input_channels.map((channel, index) => (
          <button
            key={`${channel}-${index}`}
            className={channelClassName("input-channel", channel, selectedChannelName)}
            title={channel}
            onClick={(event) => {
              event.stopPropagation();
              onSelectChannel(channel);
            }}
          >
            <Handle
              className="channel-handle input-channel-handle"
              id={`input-channel-${index}`}
              type="target"
              position={Position.Left}
              isConnectable={false}
            />
            <span className="channel-text">{channel}</span>
            {(channelDiagnosticCounts[channel] ?? 0) > 0 && (
              <DiagnosticBadge count={channelDiagnosticCounts[channel]} severity="warning" compact />
            )}
          </button>
        ))}
        {outputChannel && (
          <button
            className={channelClassName("output-channel", outputChannel, selectedChannelName)}
            title={outputChannel}
            onClick={(event) => {
              event.stopPropagation();
              onSelectChannel(outputChannel);
            }}
          >
            <span className="channel-text">{outputChannel}</span>
            <Handle
              className="channel-handle output-channel-handle"
              id="output-channel"
              type="source"
              position={Position.Right}
              isConnectable={false}
            />
            {(channelDiagnosticCounts[outputChannel] ?? 0) > 0 && (
              <DiagnosticBadge count={channelDiagnosticCounts[outputChannel]} severity="warning" compact />
            )}
          </button>
        )}
      </div>
    </div>
  );
}

type ChannelEdgeData = {
  channelName: string;
  color: string;
  laneOffset: number;
  pathState: "active" | "muted" | null;
  severity: DiagnosticSeverity | null;
};

function ChannelEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  markerEnd,
  style,
  selected,
  data,
}: EdgeProps<Edge<ChannelEdgeData>>) {
  const edgeData = data ?? {
    channelName: "",
    color: "#67e5d8",
    laneOffset: 0,
    pathState: null,
    severity: null,
  };
  const [basePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    curvature: 0.34,
  });
  const edgePath =
    edgeData.laneOffset === 0
      ? basePath
      : channelBezierPath(sourceX, sourceY, targetX, targetY, edgeData.laneOffset);

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        className={[
          "channel-edge",
          selected ? "selected" : "",
          edgeData.severity ? edgeData.severity : "",
          edgeData.pathState === "active" ? "focus-active" : "",
          edgeData.pathState === "muted" ? "focus-muted" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        style={{
          ...style,
          stroke: edgeData.color,
        }}
      />
      <EdgeLabelRenderer>
        <div
          className={[
            "edge-label",
            "nodrag",
            "nopan",
            edgeData.severity ? edgeData.severity : "",
            edgeData.pathState === "active" ? "focus-active" : "",
            edgeData.pathState === "muted" ? "focus-muted" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          style={{
            left: labelX,
            top: labelY,
            borderColor: edgeData.color,
            color: edgeData.color,
          }}
          title={edgeData.channelName}
        >
          {edgeData.channelName}
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

function channelBezierPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
  laneOffset: number,
) {
  const distance = Math.max(Math.abs(targetX - sourceX), 160);
  const controlDistance = Math.min(distance * 0.42, 260);
  const controlSourceX = sourceX + controlDistance;
  const controlTargetX = targetX - controlDistance;
  const controlSourceY = sourceY + laneOffset;
  const controlTargetY = targetY - laneOffset;

  return `M ${sourceX},${sourceY} C ${controlSourceX},${controlSourceY} ${controlTargetX},${controlTargetY} ${targetX},${targetY}`;
}

function mergeDraggedNode(nodes: Node<FlowNodeData>[], draggedNode: Node<FlowNodeData>) {
  return nodes.map((node) => (node.id === draggedNode.id ? draggedNode : node));
}

function channelClassName(kindClass: string, channelName: string, selectedChannelName: string | null) {
  return ["channel", "nodrag", kindClass, channelName === selectedChannelName ? "selected-channel" : ""]
    .filter(Boolean)
    .join(" ");
}

function timingValue(node: GraphNode, key: string, fallback: string) {
  return node.timing.find((field) => field.key === key)?.value ?? fallback;
}

function DiagnosticBadge({
  count,
  severity,
  compact = false,
}: {
  count: number;
  severity: DiagnosticSeverity;
  compact?: boolean;
}) {
  return <span className={`diagnostic-badge ${severity} ${compact ? "compact" : ""}`}>{count}</span>;
}

function colorForChannel(channelName: string) {
  let hash = 0;
  for (const character of channelName) {
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  }

  return channelPalette[hash % channelPalette.length];
}

function LaneLabels({ graph }: { graph: ResolvedPipelineGraph }) {
  return (
    <div className="lane-labels">
      {(["inputs", "pipeline_stages", "outputs"] as GraphLane[]).map((lane) => {
        const diagnostics = diagnosticsForLane(graph, lane);
        const severity = severityForDiagnostics(diagnostics);

        return (
        <div key={lane}>
          <span>{laneTitle[lane]}</span>
          {severity && <DiagnosticBadge count={diagnostics.length} severity={severity} />}
        </div>
        );
      })}
    </div>
  );
}

function diagnosticsByNodeId(graph: ResolvedPipelineGraph | null) {
  const map = new Map<string, GraphDiagnostic[]>();

  graph?.diagnostics.forEach((diagnostic) => {
    diagnostic.node_ids.forEach((nodeId) => {
      const diagnostics = map.get(nodeId) ?? [];
      diagnostics.push(diagnostic);
      map.set(nodeId, diagnostics);
    });
  });

  return map;
}

function graphStatus(
  graph: ResolvedPipelineGraph | null,
  loadState: "idle" | "loading" | "error",
  saveState: "idle" | "saving" | "error",
  error: string | null,
) {
  if (loadState === "loading") {
    return {
      severity: "loading",
      label: "Loading",
      detail: "Parsing config and resolving graph.",
    };
  }

  if (loadState === "error") {
    return {
      severity: "error",
      label: "Invalid Config",
      detail: error ?? "The config could not be loaded.",
    };
  }

  if (saveState === "saving") {
    return {
      severity: "loading",
      label: "Saving",
      detail: "Writing parameter edit and resolving graph.",
    };
  }

  if (saveState === "error") {
    return {
      severity: "error",
      label: "Edit Rejected",
      detail: error ?? "The parameter edit could not be saved.",
    };
  }

  if (!graph) {
    return {
      severity: "neutral",
      label: "No Graph",
      detail: "No resolved graph is available.",
    };
  }

  if (graph.summary.error_count > 0) {
    return {
      severity: "error",
      label: "Errors",
      detail: `${graph.summary.error_count} error diagnostics require attention.`,
    };
  }

  if (graph.summary.warning_count > 0) {
    return {
      severity: "warning",
      label: "Warnings",
      detail: `${graph.summary.warning_count} warning diagnostics found.`,
    };
  }

  return {
    severity: "valid",
    label: "Valid Graph",
    detail: "No graph diagnostics detected.",
  };
}

function diagnosticMatchesFilter(diagnostic: GraphDiagnostic, filter: DiagnosticsFilter) {
  if (filter === "errors") {
    return diagnostic.severity === "error";
  }

  if (filter === "warnings") {
    return diagnostic.severity === "warning";
  }

  return true;
}

function groupDiagnosticsBySeverity(items: { diagnostic: GraphDiagnostic; index: number }[]) {
  return (["error", "warning"] as DiagnosticSeverity[])
    .map((severity) => ({
      severity,
      items: items.filter(({ diagnostic }) => diagnostic.severity === severity),
    }))
    .filter((group) => group.items.length > 0);
}

function diagnosticCountByChannelName(graph: ResolvedPipelineGraph | null) {
  const map = new Map<string, number>();

  graph?.diagnostics.forEach((diagnostic) => {
    if (!diagnostic.channel_name) {
      return;
    }

    map.set(diagnostic.channel_name, (map.get(diagnostic.channel_name) ?? 0) + 1);
  });

  return map;
}

function diagnosticsForLane(graph: ResolvedPipelineGraph, lane: GraphLane) {
  const laneNodeIds = new Set(graph.nodes.filter((node) => node.lane === lane).map((node) => node.id));

  return graph.diagnostics.filter((diagnostic) =>
    diagnostic.node_ids.some((nodeId) => laneNodeIds.has(nodeId)),
  );
}

function severityForDiagnostics(diagnostics: GraphDiagnostic[]) {
  if (diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    return "error";
  }

  if (diagnostics.some((diagnostic) => diagnostic.severity === "warning")) {
    return "warning";
  }

  return null;
}

function diagnosticKey(diagnostic: GraphDiagnostic, index: number) {
  return [
    index,
    diagnostic.kind,
    diagnostic.severity,
    diagnostic.channel_name ?? "",
    diagnostic.node_ids.join(","),
  ].join(":");
}

function graphFocusState(
  graph: ResolvedPipelineGraph | null,
  selectedNodeId: string | null,
  selectedChannelName: string | null,
  selectedDiagnosticKey: string | null,
): FocusState | null {
  if (!graph) {
    return null;
  }

  const selectedDiagnostic =
    graph.diagnostics.find((diagnostic, index) => diagnosticKey(diagnostic, index) === selectedDiagnosticKey) ??
    null;

  if (selectedChannelName || selectedDiagnostic?.channel_name) {
    const channelName = selectedChannelName ?? selectedDiagnostic?.channel_name;
    if (!channelName) {
      return null;
    }
    const activeEdgeIds = new Set(
      graph.edges
        .filter((edge) => edge.channel_name === channelName)
        .map((edge) => edge.id),
    );
    const activeNodeIds = new Set<string>();

    graph.edges
      .filter((edge) => edge.channel_name === channelName)
      .forEach((edge) => {
        activeNodeIds.add(edge.source_node_id);
        activeNodeIds.add(edge.target_node_id);
      });

    const channel = graph.channels.find((candidate) => candidate.name === channelName);
    channel?.producer_node_ids.forEach((nodeId) => activeNodeIds.add(nodeId));
    channel?.consumer_node_ids.forEach((nodeId) => activeNodeIds.add(nodeId));
    selectedDiagnostic?.node_ids.forEach((nodeId) => activeNodeIds.add(nodeId));

    return { activeNodeIds, activeEdgeIds };
  }

  const focusNodeId = selectedNodeId ?? selectedDiagnostic?.node_ids[0] ?? null;
  if (!focusNodeId) {
    return null;
  }

  const activeNodeIds = new Set<string>([focusNodeId]);
  const activeEdgeIds = new Set<string>();
  const frontier = [focusNodeId];

  while (frontier.length > 0) {
    const nodeId = frontier.shift();
    if (!nodeId) {
      continue;
    }

    graph.edges
      .filter((edge) => edge.source_node_id === nodeId || edge.target_node_id === nodeId)
      .forEach((edge) => {
        const nextNodeId = edge.source_node_id === nodeId ? edge.target_node_id : edge.source_node_id;
        activeEdgeIds.add(edge.id);

        if (!activeNodeIds.has(nextNodeId)) {
          activeNodeIds.add(nextNodeId);
          frontier.push(nextNodeId);
        }
      });
  }

  return { activeNodeIds, activeEdgeIds };
}

function configFileName(path: string) {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

function layoutNodePositions(nodes: GraphNode[]) {
  const positions = new Map<string, { x: number; y: number }>();

  (["inputs", "pipeline_stages", "outputs"] as GraphLane[]).forEach((lane) => {
    const laneNodes = nodes
      .filter((node) => node.lane === lane)
      .sort((a, b) => a.lane_index - b.lane_index || a.id.localeCompare(b.id));
    let y = laneTop;

    laneNodes.forEach((node) => {
      positions.set(node.id, { x: laneX[lane], y });
      y += estimatedNodeHeight(node) + nodeGap;
    });
  });

  return positions;
}

function estimatedNodeHeight(node: GraphNode) {
  const channelCount = node.input_channels.length + (node.output_channel ? 1 : 0);
  const channelRows = Math.max(1, Math.ceil(channelCount / 2));
  const nameRows = Math.ceil(node.display_name.length / 26);
  const pathRows = Math.ceil(node.config_path.length / 38);

  return Math.max(168, 104 + channelRows * 34 + nameRows * 22 + pathRows * 18);
}

export default App;
