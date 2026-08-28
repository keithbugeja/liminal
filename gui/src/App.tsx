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
  const [filterText, setFilterText] = useState("");
  const [loadState, setLoadState] = useState<"idle" | "loading" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const loadGraph = useCallback(async (path: string) => {
    setLoadState("loading");
    setError(null);

    try {
      const nextGraph = await invoke<ResolvedPipelineGraph>("load_graph", { path });
      setGraph(nextGraph);
      setSelectedNodeId(nextGraph.nodes[0]?.id ?? null);
      setSelectedChannelName(null);
      setLoadState("idle");
    } catch (caught) {
      setGraph(null);
      setSelectedNodeId(null);
      setSelectedChannelName(null);
      setError(String(caught));
      setLoadState("error");
    }
  }, []);

  useEffect(() => {
    invoke<string[]>("list_example_configs")
      .then(setExampleConfigs)
      .catch(() => setExampleConfigs([]));
    loadGraph(initialConfigPath);
  }, [loadGraph]);

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
          <DiagnosticsPanel
            graph={graph}
            onSelectNode={(id) => {
              setSelectedNodeId(id);
              setSelectedChannelName(null);
            }}
          />
        </aside>

        <main className="workspace">
          <GraphCanvas
            graph={graph}
            selectedNodeId={selectedNodeId}
            selectedChannelName={selectedChannelName}
            filterText={filterText}
            onSelectNode={setSelectedNodeId}
            onSelectChannel={setSelectedChannelName}
            error={error}
            loadState={loadState}
          />
        </main>

        <InspectorPanel
          graph={graph}
          selectedNodeId={selectedNodeId}
          selectedChannelName={selectedChannelName}
          onSelectNode={setSelectedNodeId}
          onSelectChannel={setSelectedChannelName}
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
  onSelectNode,
}: {
  graph: ResolvedPipelineGraph | null;
  onSelectNode: (id: string) => void;
}) {
  const diagnostics = graph?.diagnostics ?? [];

  return (
    <section className="panel diagnostics-panel">
      <div className="panel-title">
        <AlertCircle size={16} />
        <span>Diagnostics</span>
      </div>
      {diagnostics.length === 0 ? (
        <p className="empty-state">No graph diagnostics.</p>
      ) : (
        <div className="diagnostic-list">
          {diagnostics.map((diagnostic, index) => (
            <button
              key={`${diagnostic.kind}-${diagnostic.channel_name}-${index}`}
              className={`diagnostic ${diagnostic.severity}`}
              onClick={() => {
                const [firstNode] = diagnostic.node_ids;
                if (firstNode) {
                  onSelectNode(firstNode);
                }
              }}
            >
              <span>{diagnostic.severity}</span>
              <p>{diagnostic.message}</p>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function InspectorPanel({
  graph,
  selectedNodeId,
  selectedChannelName,
  onSelectNode,
  onSelectChannel,
}: {
  graph: ResolvedPipelineGraph | null;
  selectedNodeId: string | null;
  selectedChannelName: string | null;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
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
          onSelectNode={selectNode}
        />
      ) : selectedChannelName ? (
        <MissingChannelInspector channelName={selectedChannelName} diagnostics={channelDiagnostics} />
      ) : selectedNode ? (
        <NodeInspector
          node={selectedNode}
          graph={graph}
          diagnostics={nodeDiagnostics}
          onSelectChannel={selectChannel}
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
  graph,
  diagnostics,
  onSelectChannel,
}: {
  node: GraphNode;
  graph: ResolvedPipelineGraph;
  diagnostics: GraphDiagnostic[];
  onSelectChannel: (channelName: string | null) => void;
}) {
  const incomingEdges = graph.edges.filter((edge) => edge.target_node_id === node.id);
  const outgoingEdges = graph.edges.filter((edge) => edge.source_node_id === node.id);

  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow={node.processor_type} title={node.display_name} />
      <KeyValue label="Kind" value={node.kind} />
      <KeyValue label="Config path" value={node.config_path} />
      {node.pipeline_name && <KeyValue label="Pipeline" value={node.pipeline_name} />}
      <KeyValue label="Node ID" value={node.id} />

      <InspectorSection title="Inputs">
        {node.input_channels.length === 0 ? (
          <p className="empty-state">No input channels.</p>
        ) : (
          <ChannelButtonList channels={node.input_channels} onSelectChannel={onSelectChannel} />
        )}
      </InspectorSection>

      <InspectorSection title="Output">
        {node.output_channel ? (
          <ChannelButtonList channels={[node.output_channel]} onSelectChannel={onSelectChannel} />
        ) : (
          <p className="empty-state">No output channel.</p>
        )}
      </InspectorSection>

      <InspectorSection title="Edges">
        <KeyValue label="Incoming" value={String(incomingEdges.length)} />
        <KeyValue label="Outgoing" value={String(outgoingEdges.length)} />
      </InspectorSection>

      <DiagnosticsList diagnostics={diagnostics} />
    </div>
  );
}

function ChannelInspector({
  channel,
  graph,
  diagnostics,
  onSelectNode,
}: {
  channel: GraphChannel;
  graph: ResolvedPipelineGraph;
  diagnostics: GraphDiagnostic[];
  onSelectNode: (id: string | null) => void;
}) {
  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow="Channel" title={channel.name} />
      <KeyValue label="Type" value={channel.channel_type} />
      <KeyValue label="Capacity" value={String(channel.capacity)} />

      <InspectorSection title="Producers">
        <NodeButtonList nodeIds={channel.producer_node_ids} graph={graph} onSelectNode={onSelectNode} />
      </InspectorSection>

      <InspectorSection title="Consumers">
        <NodeButtonList nodeIds={channel.consumer_node_ids} graph={graph} onSelectNode={onSelectNode} />
      </InspectorSection>

      <DiagnosticsList diagnostics={diagnostics} />
    </div>
  );
}

function MissingChannelInspector({
  channelName,
  diagnostics,
}: {
  channelName: string;
  diagnostics: GraphDiagnostic[];
}) {
  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow="Unresolved Channel" title={channelName} />
      <DiagnosticsList diagnostics={diagnostics} />
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

function InspectorSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="inspector-section">
      <h3>{title}</h3>
      {children}
    </section>
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

function DiagnosticsList({ diagnostics }: { diagnostics: GraphDiagnostic[] }) {
  return (
    <InspectorSection title="Diagnostics">
      {diagnostics.length === 0 ? (
        <p className="empty-state">No diagnostics.</p>
      ) : (
        <div className="inspector-diagnostics">
          {diagnostics.map((diagnostic, index) => (
            <div
              key={`${diagnostic.kind}-${diagnostic.channel_name}-${index}`}
              className={`inspector-diagnostic ${diagnostic.severity}`}
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
  filterText,
  onSelectNode,
  onSelectChannel,
  error,
  loadState,
}: {
  graph: ResolvedPipelineGraph | null;
  selectedNodeId: string | null;
  selectedChannelName: string | null;
  filterText: string;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
  error: string | null;
  loadState: "idle" | "loading" | "error";
}) {
  const diagnosticsByNode = useMemo(() => diagnosticsByNodeId(graph), [graph]);
  const focusState = useMemo(
    () => graphFocusState(graph, selectedNodeId, selectedChannelName),
    [graph, selectedChannelName, selectedNodeId],
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
  }, [diagnosticsByNode, filterText, focusState, graph, onSelectChannel, selectedChannelName, selectedNodeId]);

  const flowEdges = useMemo<Edge[]>(() => {
    return (graph?.edges ?? []).map((edge, index) => {
      const channel = channelByName.get(edge.channel_name);
      const color = colorForChannel(edge.channel_name);
      const laneOffset = ((index % 5) - 2) * 14;
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
      <LaneLabels />
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
  const { graphNode, diagnostics, selectedChannelName, onSelectChannel } = data;
  const hasError = diagnostics.some((diagnostic) => diagnostic.severity === "error");
  const hasWarning = diagnostics.some((diagnostic) => diagnostic.severity === "warning");
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
  const edgeData = data ?? { channelName: "", color: "#67e5d8", laneOffset: 0, pathState: null };
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

function colorForChannel(channelName: string) {
  let hash = 0;
  for (const character of channelName) {
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  }

  return channelPalette[hash % channelPalette.length];
}

function LaneLabels() {
  return (
    <div className="lane-labels">
      {(["inputs", "pipeline_stages", "outputs"] as GraphLane[]).map((lane) => (
        <div key={lane}>
          {laneTitle[lane]}
        </div>
      ))}
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

function graphFocusState(
  graph: ResolvedPipelineGraph | null,
  selectedNodeId: string | null,
  selectedChannelName: string | null,
): FocusState | null {
  if (!graph) {
    return null;
  }

  if (selectedChannelName) {
    const activeEdgeIds = new Set(
      graph.edges
        .filter((edge) => edge.channel_name === selectedChannelName)
        .map((edge) => edge.id),
    );
    const activeNodeIds = new Set<string>();

    graph.edges
      .filter((edge) => edge.channel_name === selectedChannelName)
      .forEach((edge) => {
        activeNodeIds.add(edge.source_node_id);
        activeNodeIds.add(edge.target_node_id);
      });

    const channel = graph.channels.find((candidate) => candidate.name === selectedChannelName);
    channel?.producer_node_ids.forEach((nodeId) => activeNodeIds.add(nodeId));
    channel?.consumer_node_ids.forEach((nodeId) => activeNodeIds.add(nodeId));

    return { activeNodeIds, activeEdgeIds };
  }

  if (!selectedNodeId) {
    return null;
  }

  const activeNodeIds = new Set<string>([selectedNodeId]);
  const activeEdgeIds = new Set<string>();
  const frontier = [selectedNodeId];

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
