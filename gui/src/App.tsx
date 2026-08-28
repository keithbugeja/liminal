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
  Loader2,
  RefreshCw,
  Search,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

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
          <DiagnosticsPanel graph={graph} onSelectNode={setSelectedNodeId} />
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
        className: matches ? "" : "dimmed-node",
      };
    });
  }, [diagnosticsByNode, filterText, graph, onSelectChannel, selectedChannelName, selectedNodeId]);

  const flowEdges = useMemo<Edge[]>(() => {
    return (graph?.edges ?? []).map((edge, index) => {
      const channel = channelByName.get(edge.channel_name);
      const color = colorForChannel(edge.channel_name);
      const laneOffset = ((index % 5) - 2) * 14;

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
        },
        animated: channel?.channel_type === "broadcast" || channel?.channel_type === "fanout",
        style: { stroke: color, strokeWidth: 2.1 },
      };
    });
  }, [channelByName, graph]);

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
          onNodeClick={(_, node) => onSelectNode(node.id)}
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
  const edgeData = data ?? { channelName: "", color: "#67e5d8", laneOffset: 0 };
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
        className={selected ? "channel-edge selected" : "channel-edge"}
        style={{
          ...style,
          stroke: edgeData.color,
        }}
      />
      <EdgeLabelRenderer>
        <button
          className="edge-label nodrag nopan"
          style={{
            left: labelX,
            top: labelY,
            borderColor: edgeData.color,
            color: edgeData.color,
          }}
          title={edgeData.channelName}
        >
          {edgeData.channelName}
        </button>
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
