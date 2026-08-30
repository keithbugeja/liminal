import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Background,
  BaseEdge,
  Connection,
  Controls,
  Edge,
  EdgeChange,
  EdgeLabelRenderer,
  EdgeProps,
  getBezierPath,
  Handle,
  MarkerType,
  Node,
  NodeChange,
  NodeProps,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  useStore,
  useUpdateNodeInternals,
} from "@xyflow/react";
import {
  AlertCircle,
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  Boxes,
  CircleDot,
  FilePlus,
  FileJson,
  FolderOpen,
  GitBranch,
  History,
  Info,
  Loader2,
  MoreHorizontal,
  Network,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  Square,
  Terminal,
  Trash2,
} from "lucide-react";
import { MouseEvent as ReactMouseEvent, ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";

type GraphNodeKind = "input" | "pipeline_stage" | "output";
type GraphLane = "inputs" | "pipeline_stages" | "outputs";
type DiagnosticSeverity = "warning" | "error";
type DiagnosticsFilter = "all" | "errors" | "warnings";
type SaveState = "idle" | "dirty" | "saving" | "error";
type RuntimeState = "idle" | "starting" | "running" | "stopping" | "error";
type RuntimeLogStream = "stdout" | "stderr" | "system";

type ResolvedPipelineGraph = {
  schema_version: number;
  summary: GraphSummary;
  nodes: GraphNode[];
  edges: GraphEdge[];
  channels: GraphChannel[];
  diagnostics: GraphDiagnostic[];
};

type DraftEditResult = {
  graph: ResolvedPipelineGraph;
  content: string;
};

type RuntimeLogEntry = {
  id: number;
  stream: RuntimeLogStream;
  line: string;
};

type AnsiTextState = {
  color: string | null;
  bold: boolean;
  dim: boolean;
};

type PipelineLogEvent = {
  stream: RuntimeLogStream;
  line: string;
};

type PipelineStateEvent = {
  state: "idle" | "running" | "stopped" | "error";
  message: string | null;
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
  raw_value: JsonValue;
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

type ProcessorCategory = "input" | "transform" | "aggregator" | "output";
type JsonValue = null | string | number | boolean | JsonValue[] | { [key: string]: JsonValue };
type FieldKind = "string" | "integer" | "number" | "boolean" | "enum" | "array" | "object" | "json_value";

type SchemaSpec =
  | { kind: "object"; fields: FieldSpec[] }
  | { kind: "array"; item: SchemaSpec }
  | { kind: "tagged_union"; tag: string; variants: TaggedVariantSpec[] }
  | { kind: "json_value" };

type TaggedVariantSpec = {
  tag_value: string;
  label: string;
  fields: FieldSpec[];
};

type FieldSpec = {
  key: string;
  label: string;
  kind: FieldKind;
  required: boolean;
  default_value: string | null;
  options: string[];
  help: string;
  schema: SchemaSpec | null;
  renderer: string | null;
};

type ProcessorDescriptor = {
  type_name: string;
  category: ProcessorCategory;
  display_name: string;
  description: string;
  fields: FieldSpec[];
};

type FlowNodeData = {
  graphNode: GraphNode;
  diagnostics: GraphDiagnostic[];
  channelDiagnosticCounts: Record<string, number>;
  runtimeActive: boolean;
  activeChannelNames: string[];
  connectionState: "valid" | "invalid" | null;
  selectedChannelName: string | null;
  onSelectChannel: (channelName: string) => void;
};

type FocusState = {
  activeNodeIds: Set<string>;
  activeEdgeIds: Set<string>;
};

type DeleteImpact = {
  outputChannel: string | null;
  downstreamNodes: GraphNode[];
};

type LayoutPosition = {
  x: number;
  y: number;
};

type FlowDebugSnapshot = {
  appNodes: number;
  renderedNodes: number;
  appEdges: number;
  renderedEdges: number;
  domNodes: number;
  domEdges: number;
  sourceHandles: number;
  targetHandles: number;
  edgeLayerChildren: number;
  storeEdges: number;
  storeEdgeLookup: number;
  storeNodes: number;
  storeWidth: number;
  storeHeight: number;
  nodesWithDimensions: number;
  nodesWithHandleBounds: number;
  flowErrors: string[];
  viewportTransform: string;
  viewportX: number;
  viewportY: number;
  zoom: number;
};

type ViewportSize = {
  width: number;
  height: number;
};

type PendingDelete = {
  node: GraphNode;
  impact: DeleteImpact;
};

type PendingDiscardAction = {
  title: string;
  detail: string;
  confirmLabel: string;
  run: () => void | Promise<void>;
};

type ValidationIssue = {
  path: string;
  message: string;
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
const defaultInspectorWidth = 520;
const minInspectorWidth = 330;
const maxInspectorWidth = 980;
const minGraphColumnWidth = 640;
const inspectorWidthStorageKey = "liminal.inspectorWidth";
const layoutStoragePrefix = "liminal.layout.";
const recentConfigsStorageKey = "liminal.recentConfigs";
const workspacePathStorageKey = "liminal.workspacePath";
const showExamplesStorageKey = "liminal.showExamples";
const fileSidebarWidthStorageKey = "liminal.fileSidebarWidth";
const toolsSidebarWidthStorageKey = "liminal.toolsSidebarWidth";
const fileSidebarCollapsedStorageKey = "liminal.fileSidebarCollapsed";
const toolsSidebarCollapsedStorageKey = "liminal.toolsSidebarCollapsed";
const inspectorCollapsedStorageKey = "liminal.inspectorCollapsed";
const maxRecentConfigs = 6;
const conditionOperationOptions = [
  "equals",
  "not_equals",
  "startswith",
  "endswith",
  "contains",
  ">",
  ">=",
  "<",
  "<=",
];
const nodeTypes = { liminalNode: LiminalNode };
const edgeTypes = { channelEdge: ChannelEdge };
const channelPalette = ["#67e5d8", "#8aa7ff", "#e2b24f", "#f28b82", "#b58cff", "#78d879"];
const maxRuntimeLogs = 500;
const defaultFileSidebarWidth = 320;
const defaultToolsSidebarWidth = 330;
const minFileSidebarWidth = 260;
const maxFileSidebarWidth = 520;
const minToolsSidebarWidth = 280;
const maxToolsSidebarWidth = 520;

function App() {
  const [configPath, setConfigPath] = useState(initialConfigPath);
  const [loadedConfigPath, setLoadedConfigPath] = useState<string | null>(null);
  const [exampleConfigs, setExampleConfigs] = useState<string[]>([]);
  const [recentConfigPaths, setRecentConfigPaths] = useState<string[]>(readStoredRecentConfigs);
  const [workspacePath, setWorkspacePath] = useState(readStoredWorkspacePath);
  const [workspaceConfigs, setWorkspaceConfigs] = useState<string[]>([]);
  const [showExamples, setShowExamples] = useState(readStoredShowExamples);
  const [processorDescriptors, setProcessorDescriptors] = useState<ProcessorDescriptor[]>([]);
  const [graph, setGraph] = useState<ResolvedPipelineGraph | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedChannelName, setSelectedChannelName] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [selectedDiagnosticKey, setSelectedDiagnosticKey] = useState<string | null>(null);
  const [diagnosticsFilter, setDiagnosticsFilter] = useState<DiagnosticsFilter>("all");
  const [filterText, setFilterText] = useState("");
  const [loadState, setLoadState] = useState<"idle" | "loading" | "error">("idle");
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const [runtimeState, setRuntimeState] = useState<RuntimeState>("idle");
  const [runtimeLogs, setRuntimeLogs] = useState<RuntimeLogEntry[]>([]);
  const [runtimeLogFilter, setRuntimeLogFilter] = useState<"all" | "selection">("all");
  const [error, setError] = useState<string | null>(null);
  const [savedContent, setSavedContent] = useState<string | null>(null);
  const [draftContent, setDraftContent] = useState<string | null>(null);
  const [fileSidebarWidthSetting, setFileSidebarWidthSetting] = useState(() =>
    readStoredSidebarWidth(fileSidebarWidthStorageKey, defaultFileSidebarWidth, minFileSidebarWidth, maxFileSidebarWidth),
  );
  const [toolsSidebarWidthSetting, setToolsSidebarWidthSetting] = useState(() =>
    readStoredSidebarWidth(
      toolsSidebarWidthStorageKey,
      defaultToolsSidebarWidth,
      minToolsSidebarWidth,
      maxToolsSidebarWidth,
    ),
  );
  const [inspectorWidth, setInspectorWidth] = useState(readStoredInspectorWidth);
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null);
  const [pendingDiscardAction, setPendingDiscardAction] = useState<PendingDiscardAction | null>(null);
  const [fileSidebarCollapsed, setFileSidebarCollapsed] = useState(() =>
    readStoredCollapsedState(fileSidebarCollapsedStorageKey),
  );
  const [toolsSidebarCollapsed, setToolsSidebarCollapsed] = useState(() =>
    readStoredCollapsedState(toolsSidebarCollapsedStorageKey),
  );
  const [inspectorCollapsed, setInspectorCollapsed] = useState(() =>
    readStoredCollapsedState(inspectorCollapsedStorageKey),
  );
  const [compactDensity, setCompactDensity] = useState(isCompactDensityViewport);
  const [viewportSize, setViewportSize] = useState(readViewportSize);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const isDirty = savedContent !== null && draftContent !== null && savedContent !== draftContent;

  const appendRuntimeLog = useCallback((stream: RuntimeLogStream, line: string) => {
    setRuntimeLogs((logs) =>
      [...logs, { id: Date.now() + Math.random(), stream, line }].slice(-maxRuntimeLogs),
    );
  }, []);

  const applyDraftEdit = useCallback(
    (edit: DraftEditResult, selectedNodeId: string | null, selectedEdgeId: string | null = null) => {
      setGraph(edit.graph);
      setDraftContent(edit.content);
      setSelectedNodeId(selectedNodeId);
      setSelectedChannelName(null);
      setSelectedEdgeId(selectedEdgeId);
      setSelectedDiagnosticKey(null);
      setSaveState("dirty");
    },
    [],
  );

  const loadGraph = useCallback(async (path: string) => {
    setLoadState("loading");
    setError(null);

    try {
      const nextContent = await invoke<string>("load_config_text", { path });
      const nextGraph = await invoke<ResolvedPipelineGraph>("load_graph", { path });
      setConfigPath(path);
      setLoadedConfigPath(path);
      setGraph(nextGraph);
      setSavedContent(nextContent);
      setDraftContent(nextContent);
      setSelectedNodeId(nextGraph.nodes[0]?.id ?? null);
      setSelectedChannelName(null);
      setSelectedEdgeId(null);
      setSelectedDiagnosticKey(null);
      setLoadState("idle");
      setSaveState("idle");
      setRecentConfigPaths(writeStoredRecentConfig(path));
    } catch (caught) {
      setGraph(null);
      setLoadedConfigPath(null);
      setSavedContent(null);
      setDraftContent(null);
      setSelectedNodeId(null);
      setSelectedChannelName(null);
      setSelectedEdgeId(null);
      setSelectedDiagnosticKey(null);
      setError(String(caught));
      setLoadState("error");
      setSaveState("idle");
    }
  }, []);

  const refreshWorkspaceConfigs = useCallback(async (path: string) => {
    if (!path) {
      setWorkspaceConfigs([]);
      return;
    }

    try {
      setWorkspaceConfigs(await invoke<string[]>("list_workspace_configs", { path }));
    } catch (caught) {
      setWorkspaceConfigs([]);
      setError(String(caught));
    }
  }, []);

  const requestDiscardOrRun = useCallback(
    (action: PendingDiscardAction) => {
      if (!isDirty) {
        void action.run();
        return;
      }

      setPendingDiscardAction(action);
    },
    [isDirty],
  );

  const openConfigPath = useCallback(
    (path: string) => {
      requestDiscardOrRun({
        title: "Open Config",
        detail: `Discard the current unsaved draft and open ${path}?`,
        confirmLabel: "Discard and Open",
        run: () => {
          setConfigPath(path);
          loadGraph(path);
        },
      });
    },
    [loadGraph, requestDiscardOrRun],
  );

  const chooseConfigFile = useCallback(async () => {
    let selectedPath: string | null;
    try {
      selectedPath = await invoke<string | null>("pick_config_file");
    } catch (caught) {
      setError(String(caught));
      setLoadState("error");
      return;
    }

    if (typeof selectedPath !== "string") {
      return;
    }

    setConfigPath(selectedPath);
    loadGraph(selectedPath);
  }, [loadGraph]);

  const openConfigFile = useCallback(() => {
    requestDiscardOrRun({
      title: "Open File",
      detail: "Discard the current unsaved draft and choose another TOML config?",
      confirmLabel: "Discard and Choose",
      run: chooseConfigFile,
    });
  }, [chooseConfigFile, requestDiscardOrRun]);

  const openWorkspaceFolder = useCallback(async () => {
    let selectedPath: string | null;
    try {
      selectedPath = await invoke<string | null>("pick_workspace_folder");
    } catch (caught) {
      setError(String(caught));
      setLoadState("error");
      return;
    }

    if (typeof selectedPath !== "string") {
      return;
    }

    setWorkspacePath(selectedPath);
    window.localStorage.setItem(workspacePathStorageKey, selectedPath);
    await refreshWorkspaceConfigs(selectedPath);
  }, [refreshWorkspaceConfigs]);

  const reloadConfig = useCallback(() => {
    requestDiscardOrRun({
      title: "Reload Config",
      detail: `Discard the current unsaved draft and reload ${configPath}?`,
      confirmLabel: "Discard and Reload",
      run: () => loadGraph(configPath),
    });
  }, [configPath, loadGraph, requestDiscardOrRun]);

  const copyIntoWorkspace = useCallback(async (sourcePath = configPath) => {
    if (!workspacePath) {
      setError("Choose a workspace folder before copying a config into it.");
      setSaveState("error");
      return;
    }

    setSaveState("saving");
    setError(null);

    try {
      const content =
        normalizeComparablePath(sourcePath) === normalizeComparablePath(configPath) && draftContent !== null
          ? draftContent
          : await invoke<string>("load_config_text", { path: sourcePath });
      const copiedPath = await invoke<string>("copy_config_to_workspace", {
        workspacePath,
        sourcePath,
        content,
      });
      const nextGraph = await invoke<ResolvedPipelineGraph>("load_graph", { path: copiedPath });
      setGraph(nextGraph);
      setConfigPath(copiedPath);
      setLoadedConfigPath(copiedPath);
      setDraftContent(content);
      setSavedContent(content);
      setRecentConfigPaths(writeStoredRecentConfig(copiedPath));
      setSelectedNodeId(nextGraph.nodes[0]?.id ?? null);
      setSelectedChannelName(null);
      setSelectedEdgeId(null);
      setSelectedDiagnosticKey(null);
      setSaveState("idle");
      await refreshWorkspaceConfigs(workspacePath);
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
    }
  }, [configPath, draftContent, refreshWorkspaceConfigs, workspacePath]);

  const saveDraftAs = useCallback(async () => {
    if (draftContent === null) {
      setError("No editable draft is loaded.");
      setSaveState("error");
      return;
    }

    let selectedPath: string | null;
    try {
      selectedPath = await invoke<string | null>("pick_save_config_path", {
        defaultPath: configPath.endsWith(".toml") ? configPath : "config.toml",
      });
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
      return;
    }

    if (typeof selectedPath !== "string") {
      return;
    }

    setSaveState("saving");
    setError(null);

    try {
      const nextGraph = await invoke<ResolvedPipelineGraph>("save_config_as", {
        path: selectedPath,
        content: draftContent,
      });
      setGraph(nextGraph);
      setConfigPath(selectedPath);
      setLoadedConfigPath(selectedPath);
      setSavedContent(draftContent);
      setRecentConfigPaths(writeStoredRecentConfig(selectedPath));
      setSelectedNodeId(nextGraph.nodes[0]?.id ?? null);
      setSelectedChannelName(null);
      setSelectedEdgeId(null);
      setSelectedDiagnosticKey(null);
      setSaveState("idle");
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
    }
  }, [configPath, draftContent]);

  const createNewConfigFile = useCallback(async () => {
    let selectedPath: string | null;
    try {
      selectedPath = await invoke<string | null>("pick_save_config_path", {
        defaultPath: workspacePath ? `${workspacePath}/config.toml` : "config.toml",
      });
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
      return;
    }

    if (typeof selectedPath !== "string") {
      return;
    }

    setSaveState("saving");
    setError(null);

    try {
      const emptyContent = "";
      const nextGraph = await invoke<ResolvedPipelineGraph>("save_config_as", {
        path: selectedPath,
        content: emptyContent,
      });
      setGraph(nextGraph);
      setConfigPath(selectedPath);
      setLoadedConfigPath(selectedPath);
      setSavedContent(emptyContent);
      setDraftContent(emptyContent);
      setRecentConfigPaths(writeStoredRecentConfig(selectedPath));
      setSelectedNodeId(nextGraph.nodes[0]?.id ?? null);
      setSelectedChannelName(null);
      setSelectedEdgeId(null);
      setSelectedDiagnosticKey(null);
      setSaveState("idle");
      await refreshWorkspaceConfigs(workspacePath);
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
    }
  }, [refreshWorkspaceConfigs, workspacePath]);

  const newConfigFile = useCallback(() => {
    requestDiscardOrRun({
      title: "New Config",
      detail: "Discard the current unsaved draft and create a new empty TOML config?",
      confirmLabel: "Discard and Create",
      run: createNewConfigFile,
    });
  }, [createNewConfigFile, requestDiscardOrRun]);

  useEffect(() => {
    invoke<string[]>("list_example_configs")
      .then(setExampleConfigs)
      .catch(() => setExampleConfigs([]));
    invoke<ProcessorDescriptor[]>("list_processor_descriptors")
      .then(setProcessorDescriptors)
      .catch(() => setProcessorDescriptors([]));
    invoke<string>("pipeline_runtime_state")
      .then((state) => setRuntimeState(state === "running" ? "running" : "idle"))
      .catch(() => setRuntimeState("idle"));
    loadGraph(initialConfigPath);
  }, [loadGraph]);

  useEffect(() => {
    void refreshWorkspaceConfigs(workspacePath);
  }, [refreshWorkspaceConfigs, workspacePath]);

  useEffect(() => {
    let disposed = false;
    const unlistenLog = listen<PipelineLogEvent>("pipeline://log", (event) => {
      if (!disposed) {
        appendRuntimeLog(event.payload.stream, event.payload.line);
      }
    });
    const unlistenState = listen<PipelineStateEvent>("pipeline://state", (event) => {
      if (disposed) {
        return;
      }

      setRuntimeState(event.payload.state === "running" ? "running" : "idle");
      if (event.payload.message) {
        appendRuntimeLog("system", event.payload.message);
      }
    });

    return () => {
      disposed = true;
      void unlistenLog.then((unlisten) => unlisten());
      void unlistenState.then((unlisten) => unlisten());
    };
  }, [appendRuntimeLog]);

  const startRuntime = useCallback(async () => {
    if (isDirty) {
      setError("Save or revert the current draft before running the pipeline.");
      appendRuntimeLog("system", "Run blocked: save or revert the current draft first.");
      return;
    }

    setRuntimeState("starting");
    setError(null);
    setRuntimeLogs([]);
    appendRuntimeLog("system", `Starting pipeline from ${configPath}`);

    try {
      await invoke("start_pipeline", { path: configPath });
    } catch (caught) {
      const message = String(caught);
      setRuntimeState("error");
      setError(message);
      appendRuntimeLog("system", message);
    }
  }, [appendRuntimeLog, configPath, isDirty]);

  const stopRuntime = useCallback(async () => {
    setRuntimeState("stopping");
    appendRuntimeLog("system", "Stopping pipeline...");

    try {
      await invoke("stop_pipeline");
    } catch (caught) {
      const message = String(caught);
      setRuntimeState("error");
      setError(message);
      appendRuntimeLog("system", message);
    }
  }, [appendRuntimeLog]);

  const selectedRuntimeNode =
    selectedNodeId && graph ? graph.nodes.find((node) => node.id === selectedNodeId) ?? null : null;
  const selectNode = useCallback((id: string | null) => {
    setSelectedNodeId(id);
    setSelectedChannelName(null);
    setSelectedEdgeId(null);
    setSelectedDiagnosticKey(null);
  }, []);
  const selectChannel = useCallback((channelName: string | null) => {
    setSelectedChannelName(channelName);
    setSelectedEdgeId(null);
    if (channelName) {
      setSelectedNodeId(null);
    }
    setSelectedDiagnosticKey(null);
  }, []);
  const selectEdge = useCallback((edgeId: string | null) => {
    setSelectedEdgeId(edgeId);
    if (edgeId) {
      setSelectedNodeId(null);
      setSelectedChannelName(null);
    }
    setSelectedDiagnosticKey(null);
  }, []);
  const selectDiagnostic = useCallback((diagnostic: GraphDiagnostic, index: number) => {
    setSelectedDiagnosticKey(diagnosticKey(diagnostic, index));
    setSelectedEdgeId(null);

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
      if (draftContent === null) {
        setError("No editable draft is loaded.");
        setSaveState("error");
        return;
      }

      setSaveState("saving");
      setError(null);

      try {
        const edit = await invoke<DraftEditResult>("update_node_parameter_draft", {
          content: draftContent,
          nodeId,
          parameterKey,
          value,
        });
        applyDraftEdit(edit, nodeId);
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [applyDraftEdit, draftContent],
  );
  const updateNodeParameterJson = useCallback(
    async (nodeId: string, parameterKey: string, value: JsonValue) => {
      if (draftContent === null) {
        setError("No editable draft is loaded.");
        setSaveState("error");
        return;
      }

      setSaveState("saving");
      setError(null);

      try {
        const edit = await invoke<DraftEditResult>("update_node_parameter_json_draft", {
          content: draftContent,
          nodeId,
          parameterKey,
          valueJson: JSON.stringify(value),
        });
        applyDraftEdit(edit, nodeId);
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [applyDraftEdit, draftContent],
  );
  const connectGraphNodes = useCallback(
    async (sourceNodeId: string, targetNodeId: string) => {
      if (draftContent === null) {
        setError("No editable draft is loaded.");
        setSaveState("error");
        return;
      }

      const validationMessage = connectionValidationMessage(graph, sourceNodeId, targetNodeId);
      if (validationMessage) {
        setError(validationMessage);
        setSaveState("error");
        return;
      }

      setSaveState("saving");
      setError(null);

      try {
        const edit = await invoke<DraftEditResult>("connect_nodes_draft", {
          content: draftContent,
          sourceNodeId,
          targetNodeId,
        });
        applyDraftEdit(edit, targetNodeId);
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [applyDraftEdit, draftContent, graph],
  );
  const disconnectGraphEdge = useCallback(
    async (targetNodeId: string, channelName: string) => {
      if (draftContent === null) {
        setError("No editable draft is loaded.");
        setSaveState("error");
        return;
      }

      setSaveState("saving");
      setError(null);

      try {
        const edit = await invoke<DraftEditResult>("disconnect_edge_draft", {
          content: draftContent,
          targetNodeId,
          channelName,
        });
        applyDraftEdit(edit, targetNodeId);
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [applyDraftEdit, draftContent],
  );
  const addGraphNode = useCallback(
    async (
      processorType: string,
      nodeName: string,
      processorCategory: ProcessorCategory,
      pipelineName: string | null,
    ) => {
      if (draftContent === null) {
        setError("No editable draft is loaded.");
        setSaveState("error");
        return;
      }

      setSaveState("saving");
      setError(null);

      try {
        const edit = await invoke<DraftEditResult>("add_node_draft", {
          content: draftContent,
          processorType,
          nodeName,
          pipelineName,
        });
        applyDraftEdit(edit, nodeIdForNewNode(processorCategory, nodeName, pipelineName));
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [applyDraftEdit, draftContent],
  );
  const deleteGraphNode = useCallback(
    async (nodeId: string) => {
      const node = graph?.nodes.find((candidate) => candidate.id === nodeId) ?? null;
      if (!graph || !node) {
        setError(`Node '${nodeId}' was not found.`);
        setSaveState("error");
        return;
      }

      const impact = deletionImpact(graph, node);
      setPendingDelete({ node, impact });
    },
    [graph],
  );
  const confirmDeleteGraphNode = useCallback(async () => {
    if (!pendingDelete) {
      return;
    }

    if (draftContent === null) {
      setError("No editable draft is loaded.");
      setSaveState("error");
      setPendingDelete(null);
      return;
    }

    const nodeId = pendingDelete.node.id;
    setSaveState("saving");
    setError(null);
    setPendingDelete(null);

    try {
      const edit = await invoke<DraftEditResult>("delete_node_draft", {
        content: draftContent,
        nodeId,
      });
      applyDraftEdit(edit, edit.graph.nodes[0]?.id ?? null);
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
    }
  }, [applyDraftEdit, draftContent, pendingDelete]);
  const updateNodeField = useCallback(
    async (nodeId: string, fieldKey: string, value: string) => {
      if (draftContent === null) {
        setError("No editable draft is loaded.");
        setSaveState("error");
        return;
      }

      setSaveState("saving");
      setError(null);

      try {
        const edit = await invoke<DraftEditResult>("update_node_field_draft", {
          content: draftContent,
          nodeId,
          fieldKey,
          value,
        });
        applyDraftEdit(edit, nodeId);
      } catch (caught) {
        setError(String(caught));
        setSaveState("error");
      }
    },
    [applyDraftEdit, draftContent],
  );
  const saveDraft = useCallback(async () => {
    if (draftContent === null || !isDirty) {
      return;
    }

    setSaveState("saving");
    setError(null);

    try {
      const nextGraph = await invoke<ResolvedPipelineGraph>("save_config_text", {
        path: configPath,
        content: draftContent,
      });
      setGraph(nextGraph);
      setSavedContent(draftContent);
      setSaveState("idle");
    } catch (caught) {
      setError(String(caught));
      setSaveState("error");
    }
  }, [configPath, draftContent, isDirty]);
  const revertDraft = useCallback(() => {
    if (!savedContent) {
      return;
    }

    loadGraph(configPath);
  }, [configPath, loadGraph, savedContent]);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      const hasPrimaryModifier = event.ctrlKey || event.metaKey;

      if (hasPrimaryModifier && key === "s") {
        event.preventDefault();
        if (event.shiftKey) {
          saveDraftAs();
        } else {
          saveDraft();
        }
        return;
      }

      if (hasPrimaryModifier && key === "o") {
        event.preventDefault();
        if (event.shiftKey) {
          openWorkspaceFolder();
        } else {
          openConfigFile();
        }
        return;
      }

      if (hasPrimaryModifier && key === "n") {
        event.preventDefault();
        newConfigFile();
        return;
      }

      if (hasPrimaryModifier && key === "r") {
        event.preventDefault();
        if (event.shiftKey) {
          revertDraft();
        } else {
          reloadConfig();
        }
        return;
      }

      if (hasPrimaryModifier && key === "f") {
        event.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
        return;
      }

      if (key === "escape" && !isTextEditingTarget(event.target)) {
        if (pendingDelete) {
          setPendingDelete(null);
        } else {
          setSelectedNodeId(null);
          setSelectedChannelName(null);
          setSelectedEdgeId(null);
          setSelectedDiagnosticKey(null);
        }
      }
    };

    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [
    configPath,
    newConfigFile,
    openConfigFile,
    openWorkspaceFolder,
    pendingDelete,
    reloadConfig,
    revertDraft,
    saveDraft,
    saveDraftAs,
  ]);
  const startInspectorResize = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      event.preventDefault();

      const startX = event.clientX;
      const startWidth = inspectorWidth;

      document.body.classList.add("resizing-inspector");

      const resize = (moveEvent: MouseEvent) => {
        setInspectorWidth(clampInspectorWidth(startWidth + startX - moveEvent.clientX));
      };
      const stopResize = () => {
        document.body.classList.remove("resizing-inspector");
        window.removeEventListener("mousemove", resize);
        window.removeEventListener("mouseup", stopResize);
      };

      window.addEventListener("mousemove", resize);
      window.addEventListener("mouseup", stopResize);
    },
    [inspectorWidth],
  );

  const startFileSidebarResize = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      event.preventDefault();

      const startX = event.clientX;
      const startWidth = fileSidebarWidthSetting;

      document.body.classList.add("resizing-sidebar");

      const resize = (moveEvent: MouseEvent) => {
        setFileSidebarWidthSetting(
          clampSidebarWidth(
            startWidth + moveEvent.clientX - startX,
            minFileSidebarWidth,
            maxFileSidebarWidth,
          ),
        );
      };
      const stopResize = () => {
        document.body.classList.remove("resizing-sidebar");
        window.removeEventListener("mousemove", resize);
        window.removeEventListener("mouseup", stopResize);
      };

      window.addEventListener("mousemove", resize);
      window.addEventListener("mouseup", stopResize);
    },
    [fileSidebarWidthSetting],
  );

  const startToolsSidebarResize = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      event.preventDefault();

      const startX = event.clientX;
      const startWidth = toolsSidebarWidthSetting;

      document.body.classList.add("resizing-sidebar");

      const resize = (moveEvent: MouseEvent) => {
        setToolsSidebarWidthSetting(
          clampSidebarWidth(
            startWidth + moveEvent.clientX - startX,
            minToolsSidebarWidth,
            maxToolsSidebarWidth,
          ),
        );
      };
      const stopResize = () => {
        document.body.classList.remove("resizing-sidebar");
        window.removeEventListener("mousemove", resize);
        window.removeEventListener("mouseup", stopResize);
      };

      window.addEventListener("mousemove", resize);
      window.addEventListener("mouseup", stopResize);
    },
    [toolsSidebarWidthSetting],
  );

  useEffect(() => {
    window.localStorage.setItem(inspectorWidthStorageKey, String(inspectorWidth));
  }, [inspectorWidth]);

  useEffect(() => {
    window.localStorage.setItem(fileSidebarWidthStorageKey, String(fileSidebarWidthSetting));
  }, [fileSidebarWidthSetting]);

  useEffect(() => {
    window.localStorage.setItem(toolsSidebarWidthStorageKey, String(toolsSidebarWidthSetting));
  }, [toolsSidebarWidthSetting]);

  useEffect(() => {
    window.localStorage.setItem(showExamplesStorageKey, String(showExamples));
  }, [showExamples]);

  useEffect(() => {
    window.localStorage.setItem(fileSidebarCollapsedStorageKey, String(fileSidebarCollapsed));
  }, [fileSidebarCollapsed]);

  useEffect(() => {
    window.localStorage.setItem(toolsSidebarCollapsedStorageKey, String(toolsSidebarCollapsed));
  }, [toolsSidebarCollapsed]);

  useEffect(() => {
    window.localStorage.setItem(inspectorCollapsedStorageKey, String(inspectorCollapsed));
  }, [inspectorCollapsed]);

  useEffect(() => {
    const updateViewport = () => {
      setCompactDensity(isCompactDensityViewport());
      setViewportSize(readViewportSize());
    };
    window.addEventListener("resize", updateViewport);
    return () => window.removeEventListener("resize", updateViewport);
  }, []);

  const fileSidebarWidth = fileSidebarCollapsed ? 48 : fileSidebarWidthSetting;
  const toolsSidebarWidth = toolsSidebarCollapsed ? 48 : toolsSidebarWidthSetting;
  const leftResizerWidth = fileSidebarCollapsed ? 0 : 8;
  const toolsResizerWidth = toolsSidebarCollapsed ? 0 : 8;
  const resizerWidth = inspectorCollapsed ? 0 : 8;
  const maxEffectiveInspectorWidth = Math.max(
    minInspectorWidth,
    viewportSize.width -
      fileSidebarWidth -
      leftResizerWidth -
      toolsSidebarWidth -
      toolsResizerWidth -
      resizerWidth -
      minGraphColumnWidth,
  );
  const effectiveInspectorWidth = inspectorCollapsed
    ? 48
    : Math.min(inspectorWidth, maxEffectiveInspectorWidth);

  return (
    <ReactFlowProvider>
      <div
        className={compactDensity ? "app-shell compact-density" : "app-shell"}
        style={{
          gridTemplateColumns: [
            `${fileSidebarWidth}px`,
            `${leftResizerWidth}px`,
            `${toolsSidebarWidth}px`,
            `${toolsResizerWidth}px`,
            "minmax(0, 1fr)",
            `${resizerWidth}px`,
            `${effectiveInspectorWidth}px`,
          ].join(" "),
        }}
      >
        <aside className={fileSidebarCollapsed ? "sidebar file-sidebar collapsed" : "sidebar file-sidebar"}>
          {fileSidebarCollapsed ? (
            <button
              className="sidebar-toggle collapsed-toggle"
              onClick={() => setFileSidebarCollapsed(false)}
              aria-label="Expand file sidebar"
              title="Expand file sidebar"
            >
              <PanelLeftOpen size={18} />
            </button>
          ) : (
            <>
              <div className="brand-row">
                <div className="brand-block">
                  <div className="brand-mark">
                    <GitBranch size={21} />
                  </div>
                  <div>
                    <h1>Liminal</h1>
                    <p>Pipeline graph</p>
                  </div>
                </div>
                <button
                  className="sidebar-toggle"
                  onClick={() => setFileSidebarCollapsed(true)}
                  aria-label="Collapse file sidebar"
                  title="Collapse file sidebar"
                >
                  <PanelLeftClose size={17} />
                </button>
              </div>

              <ConfigBrowserPanel
                configPath={configPath}
                draftContent={draftContent}
                exampleConfigs={exampleConfigs}
                isDirty={isDirty}
                loadState={loadState}
                recentConfigPaths={recentConfigPaths}
                saveState={saveState}
                showExamples={showExamples}
                workspacePath={workspacePath}
                workspaceConfigs={workspaceConfigs}
                onClearRecent={() => setRecentConfigPaths(clearStoredRecentConfigs())}
                onCopyIntoWorkspace={copyIntoWorkspace}
                onLoadConfig={openConfigPath}
                onNewConfig={newConfigFile}
                onOpenFile={openConfigFile}
                onOpenFolder={openWorkspaceFolder}
                onReload={reloadConfig}
                onRevert={revertDraft}
                onSave={saveDraft}
                onSaveAs={saveDraftAs}
                onToggleExamples={setShowExamples}
              />
            </>
          )}
        </aside>

        <div
          className={fileSidebarCollapsed ? "sidebar-resizer collapsed" : "sidebar-resizer"}
          onMouseDown={startFileSidebarResize}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize file sidebar"
        />

        <aside className={toolsSidebarCollapsed ? "sidebar tools-sidebar collapsed" : "sidebar tools-sidebar"}>
          {toolsSidebarCollapsed ? (
            <button
              className="sidebar-toggle collapsed-toggle"
              onClick={() => setToolsSidebarCollapsed(false)}
              aria-label="Expand tools sidebar"
              title="Expand tools sidebar"
            >
              <PanelLeftOpen size={18} />
            </button>
          ) : (
            <>
              <div className="tools-sidebar-header">
                <div className="search-row">
                  <Search size={16} />
                  <input
                    ref={searchInputRef}
                    value={filterText}
                    onChange={(event) => setFilterText(event.target.value)}
                    placeholder="Search nodes or channels"
                    aria-label="Search nodes or channels"
                  />
                </div>
                <button
                  className="sidebar-toggle"
                  onClick={() => setToolsSidebarCollapsed(true)}
                  aria-label="Collapse tools sidebar"
                  title="Collapse tools sidebar"
                >
                  <PanelLeftClose size={17} />
                </button>
              </div>

              <ToolsSidebarSection title="Add Node" icon={<Plus size={16} />}>
                <AddNodePanel
                  graph={graph}
                  hideTitle
                  processorDescriptors={processorDescriptors}
                  saveState={saveState}
                  onAddNode={addGraphNode}
                />
              </ToolsSidebarSection>

              <ToolsSidebarSection title="Graph Status" icon={<Boxes size={16} />}>
                <SummaryPanel graph={graph} hideTitle />
                <GraphStatusPanel
                  graph={graph}
                  loadState={loadState}
                  saveState={saveState}
                  error={error}
                  isDirty={isDirty}
                />
              </ToolsSidebarSection>

              <ToolsSidebarSection title="Diagnostics" icon={<AlertCircle size={16} />} grow>
                <DiagnosticsPanel
                  graph={graph}
                  hideTitle
                  filter={diagnosticsFilter}
                  onFilterChange={setDiagnosticsFilter}
                  selectedDiagnosticKey={selectedDiagnosticKey}
                  onSelectDiagnostic={selectDiagnostic}
                  onPreviousDiagnostic={() => selectDiagnosticByStep(-1)}
                  onNextDiagnostic={() => selectDiagnosticByStep(1)}
                />
              </ToolsSidebarSection>
            </>
          )}
        </aside>

        <div
          className={toolsSidebarCollapsed ? "sidebar-resizer collapsed" : "sidebar-resizer"}
          onMouseDown={startToolsSidebarResize}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize tools sidebar"
        />

        <main className="workspace">
          <GraphCanvas
            graph={graph}
            configPath={loadedConfigPath ?? configPath}
            selectedNodeId={selectedNodeId}
            selectedChannelName={selectedChannelName}
            selectedEdgeId={selectedEdgeId}
            selectedDiagnosticKey={selectedDiagnosticKey}
            filterText={filterText}
            onSelectNode={selectNode}
            onSelectChannel={selectChannel}
            onSelectEdge={selectEdge}
            onConnectNodes={connectGraphNodes}
            onDisconnectEdge={disconnectGraphEdge}
            onDeleteNode={deleteGraphNode}
            onStartRuntime={startRuntime}
            onStopRuntime={stopRuntime}
            error={error}
            loadState={loadState}
            runtimeState={runtimeState}
            runtimeLogs={runtimeLogs}
            runtimeLogFilter={runtimeLogFilter}
            selectedRuntimeNode={selectedRuntimeNode}
            selectedRuntimeChannelName={selectedChannelName}
            onRuntimeLogFilterChange={setRuntimeLogFilter}
            onClearRuntimeLogs={() => setRuntimeLogs([])}
          />
        </main>

        <div
          className={inspectorCollapsed ? "inspector-resizer collapsed" : "inspector-resizer"}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize inspector"
          title="Resize inspector"
          onMouseDown={inspectorCollapsed ? undefined : startInspectorResize}
          onDoubleClick={() => setInspectorWidth(defaultInspectorWidth)}
        />

        {inspectorCollapsed ? (
          <aside className="inspector collapsed">
            <button
              className="sidebar-toggle collapsed-toggle"
              onClick={() => setInspectorCollapsed(false)}
              aria-label="Expand inspector"
              title="Expand inspector"
            >
              <PanelRightOpen size={18} />
            </button>
          </aside>
        ) : (
          <div className="inspector-shell">
            <button
              className="sidebar-toggle inspector-collapse-button"
              onClick={() => setInspectorCollapsed(true)}
              aria-label="Collapse inspector"
              title="Collapse inspector"
            >
              <PanelRightClose size={17} />
            </button>
            <InspectorPanel
              graph={graph}
              configPath={configPath}
              processorDescriptors={processorDescriptors}
              selectedNodeId={selectedNodeId}
              selectedChannelName={selectedChannelName}
              selectedEdgeId={selectedEdgeId}
              selectedDiagnosticKey={selectedDiagnosticKey}
              onSelectNode={selectNode}
              onSelectChannel={selectChannel}
              onSelectEdge={selectEdge}
              onUpdateNodeParameter={updateNodeParameter}
              onUpdateNodeParameterJson={updateNodeParameterJson}
              onUpdateNodeField={updateNodeField}
              onDisconnectEdge={disconnectGraphEdge}
              onDeleteNode={deleteGraphNode}
              saveState={saveState}
            />
          </div>
        )}
        {pendingDelete && (
          <DeleteNodeDialog
            pendingDelete={pendingDelete}
            saveState={saveState}
            onCancel={() => setPendingDelete(null)}
            onConfirm={confirmDeleteGraphNode}
          />
        )}
        {pendingDiscardAction && (
          <UnsavedChangesDialog
            action={pendingDiscardAction}
            onCancel={() => setPendingDiscardAction(null)}
            onConfirm={async () => {
              const action = pendingDiscardAction;
              setPendingDiscardAction(null);
              await action.run();
            }}
          />
        )}
      </div>
    </ReactFlowProvider>
  );
}

function SummaryPanel({ graph, hideTitle = false }: { graph: ResolvedPipelineGraph | null; hideTitle?: boolean }) {
  const summary = graph?.summary;

  return (
    <section className="panel">
      {!hideTitle && (
        <div className="panel-title">
          <Boxes size={16} />
          <span>Graph</span>
        </div>
      )}
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
  isDirty,
}: {
  graph: ResolvedPipelineGraph | null;
  loadState: "idle" | "loading" | "error";
  saveState: SaveState;
  error: string | null;
  isDirty: boolean;
}) {
  const status = graphStatus(graph, loadState, saveState, error, isDirty);

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

function ToolsSidebarSection({
  title,
  icon,
  grow = false,
  children,
}: {
  title: string;
  icon: ReactNode;
  grow?: boolean;
  children: ReactNode;
}) {
  return (
    <section className={grow ? "tools-sidebar-section grow" : "tools-sidebar-section"}>
      <div className="tools-sidebar-section-title">
        {icon}
        <span>{title}</span>
      </div>
      <div className="tools-sidebar-section-body">{children}</div>
    </section>
  );
}

function ConfigBrowserPanel({
  configPath,
  draftContent,
  exampleConfigs,
  isDirty,
  loadState,
  recentConfigPaths,
  saveState,
  showExamples,
  workspaceConfigs,
  workspacePath,
  onClearRecent,
  onCopyIntoWorkspace,
  onLoadConfig,
  onNewConfig,
  onOpenFile,
  onOpenFolder,
  onReload,
  onRevert,
  onSave,
  onSaveAs,
  onToggleExamples,
}: {
  configPath: string;
  draftContent: string | null;
  exampleConfigs: string[];
  isDirty: boolean;
  loadState: "idle" | "loading" | "error";
  recentConfigPaths: string[];
  saveState: SaveState;
  showExamples: boolean;
  workspaceConfigs: string[];
  workspacePath: string;
  onClearRecent: () => void;
  onCopyIntoWorkspace: (sourcePath?: string) => void;
  onLoadConfig: (path: string) => void;
  onNewConfig: () => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onReload: () => void;
  onRevert: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onToggleExamples: (show: boolean) => void;
}) {
  const [filterText, setFilterText] = useState("");
  const [workspaceOpen, setWorkspaceOpen] = useState(true);
  const [recentOpen, setRecentOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const filteredWorkspaceConfigs = filterConfigPaths(workspaceConfigs, filterText);
  const filteredRecentConfigPaths = filterConfigPaths(recentConfigPaths, filterText);
  const filteredExampleConfigs = filterConfigPaths(exampleConfigs, filterText);
  const activeFileName = configFileName(configPath);
  const activeFolderName = configParentName(configPath);

  return (
    <div className="config-browser">
      <div className="file-browser-current">
        <FileJson size={17} />
        <div className="current-file-label" title={configPath}>
          <strong>{activeFileName}</strong>
          <span>{activeFolderName}</span>
        </div>
        <span className={isDirty ? "file-browser-badge dirty" : "file-browser-badge"}>
          {isDirty ? "Unsaved" : "Saved"}
        </span>
        <button
          className="file-menu-trigger"
          onClick={() => setMenuOpen((open) => !open)}
          aria-label="File actions"
          title="File actions"
        >
          <MoreHorizontal size={18} />
        </button>
        {menuOpen && (
          <div className="file-menu">
            <button
              onClick={() => {
                setMenuOpen(false);
                onReload();
              }}
            >
              {loadState === "loading" ? <Loader2 className="spin" size={15} /> : <RefreshCw size={15} />}
              <span>Reload</span>
            </button>
            <button
              disabled={!isDirty || saveState === "saving"}
              onClick={() => {
                setMenuOpen(false);
                onSave();
              }}
            >
              <Save size={15} />
              <span>Save</span>
            </button>
            <button
              disabled={!isDirty || saveState === "saving"}
              onClick={() => {
                setMenuOpen(false);
                onRevert();
              }}
            >
              <RotateCcw size={15} />
              <span>Revert</span>
            </button>
            <button
              disabled={draftContent === null || saveState === "saving"}
              onClick={() => {
                setMenuOpen(false);
                onSaveAs();
              }}
            >
              <Save size={15} />
              <span>Save As</span>
            </button>
            <button
              onClick={() => {
                setMenuOpen(false);
                onOpenFile();
              }}
            >
              <FileJson size={15} />
              <span>Open File</span>
            </button>
          </div>
        )}
      </div>

      <div className="file-filter-row">
        <Search size={17} />
        <input
          value={filterText}
          onChange={(event) => setFilterText(event.target.value)}
          placeholder="Filter files"
          aria-label="Filter files"
        />
      </div>

      <FileBrowserSection
        title="Workspace"
        icon={<FolderOpen size={14} />}
        count={workspaceConfigs.length}
        collapsed={!workspaceOpen}
        onToggle={() => setWorkspaceOpen((open) => !open)}
        meta={workspacePath ? configParentName(workspacePath) || configFileName(workspacePath) : undefined}
        metaTitle={workspacePath}
      >
        {filteredWorkspaceConfigs.length === 0 ? (
          <p className="empty-state">{workspacePath ? "No TOML files found." : "No folder selected."}</p>
        ) : (
          <ConfigFileList paths={filteredWorkspaceConfigs} activePath={configPath} onLoad={onLoadConfig} />
        )}
      </FileBrowserSection>

      <FileBrowserSection
        title="Recent"
        icon={<History size={14} />}
        count={recentConfigPaths.length}
        collapsed={!recentOpen}
        onToggle={() => setRecentOpen((open) => !open)}
        action={recentConfigPaths.length > 0 && recentOpen ? <button onClick={onClearRecent}>Clear</button> : undefined}
      >
        {filteredRecentConfigPaths.length === 0 ? (
          <p className="empty-state">No recent configs.</p>
        ) : (
          <ConfigFileList paths={filteredRecentConfigPaths} activePath={configPath} onLoad={onLoadConfig} />
        )}
      </FileBrowserSection>

      <FileBrowserSection
        title="Examples"
        icon={<FileJson size={14} />}
        count={exampleConfigs.length}
        action={
          <label className="file-toggle-row">
            <span>Read only</span>
            <input
              type="checkbox"
              checked={showExamples}
              onChange={(event) => onToggleExamples(event.target.checked)}
            />
          </label>
        }
      >
        {showExamples && (
          <ConfigFileList
            paths={filteredExampleConfigs}
            activePath={configPath}
            readOnly
            canCopy={Boolean(workspacePath) && saveState !== "saving"}
            onCopy={onCopyIntoWorkspace}
            onLoad={onLoadConfig}
          />
        )}
      </FileBrowserSection>

      <div className="file-browser-footer">
        <button onClick={onNewConfig} title="New empty TOML config">
          <FilePlus size={15} />
          <span>New file</span>
        </button>
        <button onClick={onOpenFolder}>
          <FolderOpen size={15} />
          <span>Open folder</span>
        </button>
      </div>
    </div>
  );
}

function FileBrowserSection({
  title,
  icon,
  action,
  collapsed = false,
  count,
  meta,
  metaTitle,
  onToggle,
  children,
}: {
  title: string;
  icon: ReactNode;
  action?: ReactNode;
  collapsed?: boolean;
  count?: number;
  meta?: string;
  metaTitle?: string;
  onToggle?: () => void;
  children: ReactNode;
}) {
  return (
    <section className="file-browser-section">
      <div className="file-browser-section-title">
        <button className="file-browser-section-toggle" onClick={onToggle} disabled={!onToggle}>
          <span className={collapsed ? "section-caret" : "section-caret open"} />
          {icon}
          <span>{title}</span>
        </button>
        <div className="file-browser-section-meta">
          {action}
          {meta ? <strong title={metaTitle}>{meta}</strong> : null}
          {count !== undefined && <em>{count}</em>}
        </div>
      </div>
      {!collapsed && children}
    </section>
  );
}

function ConfigFileList({
  paths,
  activePath,
  canCopy = false,
  onLoad,
  onCopy,
  readOnly = false,
}: {
  paths: string[];
  activePath: string;
  canCopy?: boolean;
  onLoad: (path: string) => void;
  onCopy?: (path: string) => void;
  readOnly?: boolean;
}) {
  return (
    <div className="example-list compact">
      {paths.map((path) => (
        <div
          key={path}
          className={[
            "example",
            path === activePath ? "active" : "",
            readOnly ? "read-only" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          title={path}
        >
          <button className="example-load" onClick={() => onLoad(path)}>
            <span>{configFileName(path)}</span>
            <strong>{configParentName(path)}</strong>
          </button>
          {canCopy && onCopy && (
            <button className="example-copy" onClick={() => onCopy(path)}>
              Copy in
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

function AddNodePanel({
  graph,
  hideTitle = false,
  processorDescriptors,
  saveState,
  onAddNode,
}: {
  graph: ResolvedPipelineGraph | null;
  hideTitle?: boolean;
  processorDescriptors: ProcessorDescriptor[];
  saveState: SaveState;
  onAddNode: (
    processorType: string,
    nodeName: string,
    processorCategory: ProcessorCategory,
    pipelineName: string | null,
  ) => Promise<void>;
}) {
  const descriptors = useMemo(
    () =>
      [...processorDescriptors].sort(
        (left, right) =>
          categoryOrder(left.category) - categoryOrder(right.category) ||
          left.display_name.localeCompare(right.display_name),
      ),
    [processorDescriptors],
  );
  const [processorType, setProcessorType] = useState("");
  const [nodeName, setNodeName] = useState("");
  const [pipelineName, setPipelineName] = useState("");
  const selectedDescriptor =
    descriptors.find((descriptor) => descriptor.type_name === processorType) ?? descriptors[0] ?? null;
  const pipelineOptions = useMemo(() => pipelineNames(graph), [graph]);
  const needsPipeline =
    selectedDescriptor?.category === "transform" || selectedDescriptor?.category === "aggregator";
  const normalizedNodeName = nodeName.trim();
  const normalizedPipelineName = pipelineName.trim() || pipelineOptions[0] || "default_pipeline";
  const validationMessage = selectedDescriptor
    ? addNodeValidationMessage(graph, selectedDescriptor.category, normalizedNodeName, normalizedPipelineName)
    : "Processor descriptors are not loaded.";

  useEffect(() => {
    if (!selectedDescriptor) {
      return;
    }

    if (processorType !== selectedDescriptor.type_name) {
      setProcessorType(selectedDescriptor.type_name);
    }
  }, [processorType, selectedDescriptor]);

  useEffect(() => {
    if (selectedDescriptor) {
      setNodeName(defaultNodeName(graph, selectedDescriptor));
    }
  }, [graph, selectedDescriptor?.type_name]);

  useEffect(() => {
    setPipelineName((currentName) => currentName || pipelineOptions[0] || "default_pipeline");
  }, [pipelineOptions]);

  return (
    <section className="panel add-node-panel">
      {!hideTitle && (
        <div className="panel-title">
          <Plus size={16} />
          <span>Add Node</span>
        </div>
      )}
      <form
        className="add-node-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!selectedDescriptor || validationMessage) {
            return;
          }

          onAddNode(
            selectedDescriptor.type_name,
            normalizedNodeName,
            selectedDescriptor.category,
            needsPipeline ? normalizedPipelineName : null,
          );
        }}
      >
        <label>
          <span>Processor</span>
          <select
            value={selectedDescriptor?.type_name ?? ""}
            onChange={(event) => setProcessorType(event.target.value)}
            disabled={descriptors.length === 0}
          >
            {descriptors.map((descriptor) => (
              <option key={descriptor.type_name} value={descriptor.type_name}>
                {descriptor.display_name}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Name</span>
          <input
            value={nodeName}
            onChange={(event) => setNodeName(event.target.value)}
            placeholder="node_name"
            aria-label="New node name"
          />
        </label>
        {needsPipeline && (
          <label>
            <span>Pipeline</span>
            <input
              value={pipelineName}
              onChange={(event) => setPipelineName(event.target.value)}
              list="pipeline-options"
              placeholder="default_pipeline"
              aria-label="Pipeline name"
            />
            <datalist id="pipeline-options">
              {pipelineOptions.map((name) => (
                <option key={name} value={name} />
              ))}
            </datalist>
          </label>
        )}
        <div className="add-node-footer">
          <span>{selectedDescriptor?.category ?? "processor"}</span>
          <button disabled={saveState === "saving" || Boolean(validationMessage)}>
            {saveState === "saving" ? "Adding" : "Add"}
          </button>
        </div>
        {validationMessage && <p className="form-hint">{validationMessage}</p>}
      </form>
    </section>
  );
}

function DiagnosticsPanel({
  graph,
  hideTitle = false,
  filter,
  onFilterChange,
  selectedDiagnosticKey,
  onSelectDiagnostic,
  onPreviousDiagnostic,
  onNextDiagnostic,
}: {
  graph: ResolvedPipelineGraph | null;
  hideTitle?: boolean;
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
      {!hideTitle && (
        <div className="panel-title">
          <AlertCircle size={16} />
          <span>Diagnostics</span>
        </div>
      )}
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
  processorDescriptors,
  selectedNodeId,
  selectedChannelName,
  selectedEdgeId,
  selectedDiagnosticKey,
  onSelectNode,
  onSelectChannel,
  onSelectEdge,
  onUpdateNodeParameter,
  onUpdateNodeParameterJson,
  onUpdateNodeField,
  onDisconnectEdge,
  onDeleteNode,
  saveState,
}: {
  graph: ResolvedPipelineGraph | null;
  configPath: string;
  processorDescriptors: ProcessorDescriptor[];
  selectedNodeId: string | null;
  selectedChannelName: string | null;
  selectedEdgeId: string | null;
  selectedDiagnosticKey: string | null;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
  onSelectEdge: (edgeId: string | null) => void;
  onUpdateNodeParameter: (nodeId: string, parameterKey: string, value: string) => Promise<void>;
  onUpdateNodeParameterJson: (nodeId: string, parameterKey: string, value: JsonValue) => Promise<void>;
  onUpdateNodeField: (nodeId: string, fieldKey: string, value: string) => Promise<void>;
  onDisconnectEdge: (targetNodeId: string, channelName: string) => Promise<void>;
  onDeleteNode: (nodeId: string) => Promise<void>;
  saveState: SaveState;
}) {
  const selectedNode = graph?.nodes.find((node) => node.id === selectedNodeId) ?? null;
  const selectedEdge = graph?.edges.find((edge) => edge.id === selectedEdgeId) ?? null;
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
  const disconnectEdge = useCallback(
    async (edge: GraphEdge) => {
      await onDisconnectEdge(edge.target_node_id, edge.channel_name);
      onSelectEdge(null);
    },
    [onDisconnectEdge, onSelectEdge],
  );

  return (
    <aside className="inspector">
      <div className="inspector-header">
        <Info size={17} />
        <span>Inspector</span>
      </div>

      {!graph ? (
        <p className="empty-state">No graph loaded.</p>
      ) : selectedEdge ? (
        <EdgeInspector
          edge={selectedEdge}
          graph={graph}
          saveState={saveState}
          onSelectNode={selectNode}
          onSelectChannel={selectChannel}
          onDisconnectEdge={disconnectEdge}
        />
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
          processorDescriptors={processorDescriptors}
          diagnostics={nodeDiagnostics}
          selectedDiagnostic={selectedDiagnostic}
          onSelectChannel={selectChannel}
          onUpdateParameter={onUpdateNodeParameter}
          onUpdateParameterJson={onUpdateNodeParameterJson}
          onUpdateField={onUpdateNodeField}
          onDeleteNode={onDeleteNode}
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
  processorDescriptors,
  diagnostics,
  selectedDiagnostic,
  onSelectChannel,
  onUpdateParameter,
  onUpdateParameterJson,
  onUpdateField,
  onDeleteNode,
  saveState,
}: {
  node: GraphNode;
  configPath: string;
  graph: ResolvedPipelineGraph;
  processorDescriptors: ProcessorDescriptor[];
  diagnostics: GraphDiagnostic[];
  selectedDiagnostic: GraphDiagnostic | null;
  onSelectChannel: (channelName: string | null) => void;
  onUpdateParameter: (nodeId: string, parameterKey: string, value: string) => Promise<void>;
  onUpdateParameterJson: (nodeId: string, parameterKey: string, value: JsonValue) => Promise<void>;
  onUpdateField: (nodeId: string, fieldKey: string, value: string) => Promise<void>;
  onDeleteNode: (nodeId: string) => Promise<void>;
  saveState: SaveState;
}) {
  const incomingEdges = graph.edges.filter((edge) => edge.target_node_id === node.id);
  const outgoingEdges = graph.edges.filter((edge) => edge.source_node_id === node.id);
  const outputChannel = node.output_channel
    ? graph.channels.find((channel) => channel.name === node.output_channel) ?? null
    : null;
  const processorDescriptor =
    processorDescriptors.find((descriptor) => descriptor.type_name === node.processor_type) ?? null;
  const knownParameterCount = processorDescriptor?.fields.length ?? 0;
  const parameterFields = processorDescriptor?.fields ?? [];
  const missingParameterFields = parameterFields.filter(
    (field) => !node.parameters.some((parameter) => parameter.key === field.key),
  );

  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow={node.processor_type} title={node.display_name} />
      <div className="edge-actions">
        <button
          className="danger-action"
          disabled={saveState === "saving"}
          onClick={() => onDeleteNode(node.id)}
        >
          <Trash2 size={15} />
          <span>{saveState === "saving" ? "Deleting" : "Delete Node"}</span>
        </button>
      </div>
      {selectedDiagnostic && <SelectedDiagnostic diagnostic={selectedDiagnostic} />}
      <InspectorSection title="Overview" defaultOpen>
        {processorDescriptor && (
          <DescriptorSummary descriptor={processorDescriptor} configuredCount={node.parameters.length} />
        )}
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

      <InspectorSection
        title="Parameters"
        badge={String(Math.max(node.parameters.length, knownParameterCount))}
      >
        {node.parameters.length === 0 && missingParameterFields.length === 0 ? (
          <p className="empty-state">No processor parameters.</p>
        ) : (
          <div className="parameter-list">
            {node.parameters.map((parameter) => (
              <ParameterRow
                key={parameter.key}
                nodeId={node.id}
                parameter={parameter}
                fieldSpec={parameterFields.find((field) => field.key === parameter.key)}
                saveState={saveState}
                onUpdateParameter={onUpdateParameter}
                onUpdateParameterJson={onUpdateParameterJson}
              />
            ))}
            {missingParameterFields.map((field) => (
              <MissingParameterRow
                key={field.key}
                nodeId={node.id}
                field={field}
                saveState={saveState}
                onUpdateParameterJson={onUpdateParameterJson}
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
  fieldSpec,
  saveState,
  onUpdateParameter,
  onUpdateParameterJson,
}: {
  nodeId: string;
  parameter: GraphParameter;
  fieldSpec?: FieldSpec;
  saveState: SaveState;
  onUpdateParameter: (nodeId: string, parameterKey: string, value: string) => Promise<void>;
  onUpdateParameterJson: (nodeId: string, parameterKey: string, value: JsonValue) => Promise<void>;
}) {
  const [draftValue, setDraftValue] = useState(parameter.value);
  const isDirty = draftValue !== parameter.value;
  const label = fieldSpec?.label ?? parameter.key;
  const fieldKind = editableKindForParameter(parameter, fieldSpec);
  const valueKind = fieldSpec?.kind ?? parameter.value_kind;
  const options = fieldSpec ? selectOptionsForValue(fieldSpec, draftValue) : [];

  useEffect(() => {
    setDraftValue(parameter.value);
  }, [parameter.value]);

  if (!parameter.editable) {
    return (
      <div
        className={
          fieldSpec?.renderer === "rule_builder"
            ? "parameter-row read-only rule-parameter-row"
            : "parameter-row read-only"
        }
      >
        <div className="parameter-label">
          <strong title={parameter.key}>{label}</strong>
          <span>{valueKind}</span>
        </div>
        {fieldSpec?.help && <p className="parameter-help">{fieldSpec.help}</p>}
        {fieldSpec?.renderer === "rule_builder" && fieldSpec.schema ? (
          <RuleParameterEditor
            nodeId={nodeId}
            parameterKey={parameter.key}
            value={parameter.raw_value}
            schema={fieldSpec.schema}
            saveState={saveState}
            onUpdateParameterJson={onUpdateParameterJson}
          />
        ) : fieldSpec?.renderer === "string_array" ? (
          <StringArrayParameterEditor
            nodeId={nodeId}
            parameterKey={parameter.key}
            value={parameter.raw_value}
            saveState={saveState}
            onUpdateParameterJson={onUpdateParameterJson}
          />
        ) : fieldSpec?.schema ? (
          <NestedParameterPreview value={parameter.raw_value} schema={fieldSpec.schema} />
        ) : (
          <pre>{parameter.value}</pre>
        )}
      </div>
    );
  }

  return (
    <div className={isDirty ? "parameter-row dirty" : "parameter-row"}>
      <div className="parameter-label">
        <strong title={parameter.key}>{label}</strong>
        <span>{valueKind}</span>
      </div>
      {fieldSpec?.help && <p className="parameter-help">{fieldSpec.help}</p>}
      {fieldKind === "enum" ? (
        <select value={draftValue} onChange={(event) => setDraftValue(event.target.value)}>
          {options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      ) : fieldKind === "boolean" ? (
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
          type={fieldKind === "number" ? "number" : "text"}
          onChange={(event) => setDraftValue(event.target.value)}
        />
      )}
      <button
        className="parameter-save"
        disabled={!isDirty || saveState === "saving"}
        onClick={() => onUpdateParameter(nodeId, parameter.key, draftValue)}
      >
        {saveState === "saving" ? "Saving" : "Save"}
      </button>
    </div>
  );
}

function MissingParameterRow({
  nodeId,
  field,
  saveState,
  onUpdateParameterJson,
}: {
  nodeId: string;
  field: FieldSpec;
  saveState: SaveState;
  onUpdateParameterJson: (nodeId: string, parameterKey: string, value: JsonValue) => Promise<void>;
}) {
  if (field.renderer === "rule_builder" && field.schema) {
    return (
      <div className="parameter-row read-only missing-parameter rule-parameter-row">
        <div className="parameter-label">
          <strong title={field.key}>{field.label}</strong>
          <span>{field.kind}</span>
        </div>
        {field.help && <p className="parameter-help">{field.help}</p>}
        <RuleParameterEditor
          nodeId={nodeId}
          parameterKey={field.key}
          value={[]}
          schema={field.schema}
          saveState={saveState}
          onUpdateParameterJson={onUpdateParameterJson}
        />
      </div>
    );
  }

  if (field.renderer === "string_array") {
    return (
      <div className="parameter-row read-only missing-parameter rule-parameter-row">
        <div className="parameter-label">
          <strong title={field.key}>{field.label}</strong>
          <span>{field.kind}</span>
        </div>
        {field.help && <p className="parameter-help">{field.help}</p>}
        <StringArrayParameterEditor
          nodeId={nodeId}
          parameterKey={field.key}
          value={defaultValueForField(field)}
          saveState={saveState}
          onUpdateParameterJson={onUpdateParameterJson}
        />
      </div>
    );
  }

  const defaultValue = defaultValueForField(field);

  return (
    <div className="parameter-row read-only missing-parameter">
      <div className="parameter-label">
        <strong title={field.key}>{field.label}</strong>
        <span>{field.kind}</span>
      </div>
      {field.help && <p className="parameter-help">{field.help}</p>}
      <div className="parameter-default">
        <span>{field.required ? "required" : "default"}</span>
        <strong>{field.default_value ?? "not set"}</strong>
      </div>
      <button
        className="parameter-placeholder-action"
        disabled={saveState === "saving"}
        onClick={() => onUpdateParameterJson(nodeId, field.key, defaultValue)}
      >
        {field.required ? "Configure" : "Set default"}
      </button>
    </div>
  );
}

function StringArrayParameterEditor({
  nodeId,
  parameterKey,
  value,
  saveState,
  onUpdateParameterJson,
}: {
  nodeId: string;
  parameterKey: string;
  value: JsonValue;
  saveState: SaveState;
  onUpdateParameterJson: (nodeId: string, parameterKey: string, value: JsonValue) => Promise<void>;
}) {
  const arrayValue = useMemo(
    () => (Array.isArray(value) ? value.map((item) => formatJsonValue(item)) : []),
    [value],
  );
  const [draftItems, setDraftItems] = useState<string[]>(arrayValue);
  const isDirty = JSON.stringify(draftItems) !== JSON.stringify(arrayValue);

  useEffect(() => {
    setDraftItems(arrayValue);
  }, [arrayValue]);

  return (
    <div className="string-array-editor">
      <div className="string-array-list">
        {draftItems.length === 0 ? (
          <p className="empty-state">No values configured.</p>
        ) : (
          draftItems.map((item, index) => (
            <div className="string-array-row" key={index}>
              <input
                value={item}
                aria-label={`${parameterKey} item ${index + 1}`}
                onChange={(event) =>
                  setDraftItems((currentItems) =>
                    currentItems.map((currentItem, currentIndex) =>
                      currentIndex === index ? event.target.value : currentItem,
                    ),
                  )
                }
              />
              <button
                className="icon-button danger"
                onClick={() =>
                  setDraftItems((currentItems) =>
                    currentItems.filter((_, currentIndex) => currentIndex !== index),
                  )
                }
                aria-label={`Remove ${parameterKey} item ${index + 1}`}
                title="Remove"
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))
        )}
      </div>
      <div className="string-array-actions">
        <button
          className="compact-add-button"
          onClick={() => setDraftItems((currentItems) => [...currentItems, ""])}
        >
          Add
        </button>
        <button disabled={!isDirty || saveState === "saving"} onClick={() => setDraftItems(arrayValue)}>
          Revert
        </button>
        <button
          disabled={!isDirty || saveState === "saving"}
          onClick={() => onUpdateParameterJson(nodeId, parameterKey, draftItems)}
        >
          {saveState === "saving" ? "Saving" : "Save"}
        </button>
      </div>
    </div>
  );
}

function RuleParameterEditor({
  nodeId,
  parameterKey,
  value,
  schema,
  saveState,
  onUpdateParameterJson,
}: {
  nodeId: string;
  parameterKey: string;
  value: JsonValue;
  schema: SchemaSpec;
  saveState: SaveState;
  onUpdateParameterJson: (nodeId: string, parameterKey: string, value: JsonValue) => Promise<void>;
}) {
  const [draftRules, setDraftRules] = useState<JsonValue[]>(Array.isArray(value) ? value : []);
  const actionSchema = ruleActionSchema(schema);
  const isDirty = JSON.stringify(draftRules) !== JSON.stringify(Array.isArray(value) ? value : []);
  const validationIssues = useMemo(
    () => validateRules(draftRules, actionSchema),
    [actionSchema, draftRules],
  );

  useEffect(() => {
    setDraftRules(Array.isArray(value) ? value : []);
  }, [value]);

  return (
    <div className="rule-editor">
      <div className="rule-editor-toolbar">
        <button
          onClick={() =>
            setDraftRules((currentRules) => [
              ...currentRules,
              defaultRule(actionSchema),
            ])
          }
        >
          Add Rule
        </button>
        <button disabled={!isDirty || saveState === "saving"} onClick={() => setDraftRules(Array.isArray(value) ? value : [])}>
          Revert
        </button>
        <button
          disabled={!isDirty || saveState === "saving" || validationIssues.length > 0}
          onClick={() => onUpdateParameterJson(nodeId, parameterKey, draftRules)}
        >
          {saveState === "saving" ? "Saving" : "Save Rules"}
        </button>
      </div>
      {validationIssues.length > 0 && <RuleValidationSummary issues={validationIssues} />}
      {draftRules.length === 0 ? (
        <p className="empty-state">No rules configured.</p>
      ) : (
        draftRules.map((rule, index) => (
          <RuleCard
            key={index}
            rule={rule}
            index={index}
            actionSchema={actionSchema}
            issues={validationIssues.filter((issue) => issue.path.startsWith(`rules.${index}.`))}
            onChange={(nextRule) => {
              setDraftRules((currentRules) =>
                currentRules.map((currentRule, currentIndex) => (currentIndex === index ? nextRule : currentRule)),
              );
            }}
            onMove={(direction) => setDraftRules((currentRules) => moveArrayItem(currentRules, index, direction))}
            onRemove={() =>
              setDraftRules((currentRules) => currentRules.filter((_, currentIndex) => currentIndex !== index))
            }
            canMoveUp={index > 0}
            canMoveDown={index < draftRules.length - 1}
          />
        ))
      )}
    </div>
  );
}

function RuleCard({
  rule,
  index,
  actionSchema,
  issues,
  onChange,
  onMove,
  onRemove,
  canMoveUp,
  canMoveDown,
}: {
  rule: JsonValue;
  index: number;
  actionSchema: Extract<SchemaSpec, { kind: "tagged_union" }> | null;
  issues: ValidationIssue[];
  onChange: (rule: JsonValue) => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  canMoveUp: boolean;
  canMoveDown: boolean;
}) {
  const ruleObject = isJsonObject(rule) ? rule : {};
  const condition = isJsonObject(ruleObject.condition) ? ruleObject.condition : {};
  const actions = Array.isArray(ruleObject.actions) ? ruleObject.actions : [];
  const elseActions = Array.isArray(ruleObject.else_actions) ? ruleObject.else_actions : [];

  return (
    <div className="rule-card">
      <div className="rule-card-header">
        <span>Rule {index + 1}</span>
        <strong>{ruleSummary(condition, actions.length, elseActions.length)}</strong>
        <div className="rule-button-group">
          <button
            className="icon-button"
            disabled={!canMoveUp}
            onClick={() => onMove(-1)}
            aria-label={`Move rule ${index + 1} up`}
            title="Move up"
          >
            <ArrowUp size={13} />
          </button>
          <button
            className="icon-button"
            disabled={!canMoveDown}
            onClick={() => onMove(1)}
            aria-label={`Move rule ${index + 1} down`}
            title="Move down"
          >
            <ArrowDown size={13} />
          </button>
          <button className="icon-button danger" onClick={onRemove} aria-label={`Remove rule ${index + 1}`} title="Remove">
            <Trash2 size={13} />
          </button>
        </div>
      </div>

      <div className="rule-condition">
        <RuleInput
          label="Field"
          value={formatJsonValue(condition.field_path ?? "")}
          issue={issueForPath(issues, `condition.field_path`)}
          onChange={(nextValue) =>
            onChange(setObjectValue(ruleObject, ["condition", "field_path"], nextValue))
          }
        />
        <RuleSelect
          label="Operation"
          value={typeof condition.operation === "string" ? condition.operation : "equals"}
          options={conditionOperationOptions}
          issue={issueForPath(issues, `condition.operation`)}
          onChange={(nextValue) =>
            onChange(setObjectValue(ruleObject, ["condition", "operation"], nextValue))
          }
        />
        <RuleInput
          label="Value"
          value={formatJsonValue(condition.value ?? "")}
          issue={issueForPath(issues, `condition.value`)}
          onChange={(nextValue) =>
            onChange(setObjectValue(ruleObject, ["condition", "value"], parseJsonLikeValue(nextValue)))
          }
        />
      </div>

      <RuleActionList
        title="Actions"
        actions={actions}
        actionSchema={actionSchema}
        issues={issues.filter((issue) => issue.path.startsWith("actions."))}
        onChange={(nextActions) => onChange({ ...ruleObject, actions: nextActions })}
      />
      <RuleActionList
        title="Else Actions"
        actions={elseActions}
        actionSchema={actionSchema}
        issues={issues.filter((issue) => issue.path.startsWith("else_actions."))}
        onChange={(nextActions) => onChange({ ...ruleObject, else_actions: nextActions })}
      />
    </div>
  );
}

function RuleActionList({
  title,
  actions,
  actionSchema,
  issues,
  onChange,
}: {
  title: string;
  actions: JsonValue[];
  actionSchema: Extract<SchemaSpec, { kind: "tagged_union" }> | null;
  issues: ValidationIssue[];
  onChange: (actions: JsonValue[]) => void;
}) {
  return (
    <div className="rule-action-section">
      <div className="rule-action-section-header">
        <div className="rule-action-section-heading">
          <span>{title}</span>
          <strong>{actions.length}</strong>
        </div>
        <button
          className="compact-add-button"
          disabled={!actionSchema}
          onClick={() =>
            actionSchema && onChange([...actions, defaultActionForVariant(actionSchema, actionSchema.variants[0]?.tag_value ?? "")])
          }
        >
          Add
        </button>
      </div>
      {actions.length === 0 ? (
        <p className="empty-state">None.</p>
      ) : (
        <div className="rule-action-list">
          {actions.map((action, index) => (
            <RuleActionCard
              key={index}
              action={action}
              index={index}
              actionSchema={actionSchema}
              issues={issues
                .filter((issue) => issue.path.startsWith(`${index}.`))
                .map((issue) => ({ ...issue, path: issue.path.replace(`${index}.`, "") }))}
              onChange={(nextAction) => {
                onChange(
                  actions.map((currentAction, currentIndex) =>
                    currentIndex === index ? nextAction : currentAction,
                  ),
                );
              }}
              onMove={(direction) => onChange(moveArrayItem(actions, index, direction))}
              onRemove={() => onChange(actions.filter((_, currentIndex) => currentIndex !== index))}
              canMoveUp={index > 0}
              canMoveDown={index < actions.length - 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function RuleActionCard({
  action,
  index,
  actionSchema,
  issues,
  onChange,
  onMove,
  onRemove,
  canMoveUp,
  canMoveDown,
}: {
  action: JsonValue;
  index: number;
  actionSchema: Extract<SchemaSpec, { kind: "tagged_union" }> | null;
  issues: ValidationIssue[];
  onChange: (action: JsonValue) => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  canMoveUp: boolean;
  canMoveDown: boolean;
}) {
  const actionObject = isJsonObject(action) ? action : {};
  const type = typeof actionObject.type === "string" ? actionObject.type : "";
  const variant = actionSchema?.variants.find((candidate) => candidate.tag_value === type);
  const fields = variant?.fields ?? Object.keys(actionObject)
    .filter((key) => key !== "type")
    .map((key) => ({
      key,
      label: labelFromKey(key),
      kind: "json_value" as FieldKind,
      required: false,
      default_value: null,
      options: [],
      help: "",
      schema: null,
      renderer: null,
    }));

  return (
    <div className="rule-action-card">
      <div className="rule-action-title">
        <span>Action {index + 1}</span>
        {actionSchema ? (
          <select
            className={issueForPath(issues, "type") ? "invalid" : ""}
            value={type}
            onChange={(event) => onChange(defaultActionForVariant(actionSchema, event.target.value))}
          >
            {actionSchema.variants.map((candidate) => (
              <option key={candidate.tag_value} value={candidate.tag_value}>
                {candidate.label}
              </option>
            ))}
          </select>
        ) : (
          <strong>{variant?.label ?? labelFromKey(type || "action")}</strong>
        )}
      </div>
      <div className="rule-action-fields">
        {fields.map((field) => (
          <RuleFieldEditor
            key={field.key}
            field={field}
            value={actionObject[field.key] ?? ""}
            issue={issueForPath(issues, field.key)}
            onChange={(nextValue) => onChange({ ...actionObject, [field.key]: nextValue })}
          />
        ))}
      </div>
      <div className="rule-button-group rule-row-controls">
        <button
          className="icon-button"
          disabled={!canMoveUp}
          onClick={() => onMove(-1)}
          aria-label={`Move action ${index + 1} up`}
          title="Move up"
        >
          <ArrowUp size={13} />
        </button>
        <button
          className="icon-button"
          disabled={!canMoveDown}
          onClick={() => onMove(1)}
          aria-label={`Move action ${index + 1} down`}
          title="Move down"
        >
          <ArrowDown size={13} />
        </button>
        <button className="icon-button danger" onClick={onRemove} aria-label={`Remove action ${index + 1}`} title="Remove">
          <Trash2 size={13} />
        </button>
      </div>
    </div>
  );
}

function RuleValidationSummary({ issues }: { issues: ValidationIssue[] }) {
  return (
    <div className="rule-validation">
      <strong>{issues.length} issue{issues.length === 1 ? "" : "s"}</strong>
      {issues.slice(0, 4).map((issue) => (
        <p key={`${issue.path}-${issue.message}`}>{issue.message}</p>
      ))}
      {issues.length > 4 && <p>{issues.length - 4} more.</p>}
    </div>
  );
}

function RuleFieldEditor({
  field,
  value,
  issue,
  onChange,
}: {
  field: FieldSpec;
  value: JsonValue;
  issue?: ValidationIssue;
  onChange: (value: JsonValue) => void;
}) {
  if (field.kind === "boolean") {
    return (
      <label className={["rule-datum", "rule-checkbox", issue ? "invalid" : ""].join(" ")}>
        <span>{field.label}</span>
        <input
          type="checkbox"
          checked={value === true}
          onChange={(event) => onChange(event.target.checked)}
        />
        {issue && <small>{issue.message}</small>}
      </label>
    );
  }

  if (field.kind === "enum" && field.options.length > 0) {
    return (
      <label className={["rule-datum", issue ? "invalid" : ""].join(" ")}>
        <span>{field.label}</span>
        <select
          value={typeof value === "string" ? value : field.options[0]}
          onChange={(event) => onChange(event.target.value)}
        >
          {field.options.map((option) => (
            <option key={option} value={option}>
            {option}
          </option>
        ))}
        </select>
        {issue && <small>{issue.message}</small>}
      </label>
    );
  }

  return (
    <RuleInput
      label={field.label}
      value={formatJsonValue(value)}
      issue={issue}
      onChange={(nextValue) =>
        onChange(field.kind === "json_value" ? parseJsonLikeValue(nextValue) : nextValue)
      }
    />
  );
}

function RuleInput({
  label,
  value,
  issue,
  onChange,
}: {
  label: string;
  value: string;
  issue?: ValidationIssue;
  onChange: (value: string) => void;
}) {
  return (
    <label className={["rule-datum", issue ? "invalid" : ""].join(" ")}>
      <span>{label}</span>
      <input className={issue ? "invalid" : ""} value={value} onChange={(event) => onChange(event.target.value)} />
      {issue && <small>{issue.message}</small>}
    </label>
  );
}

function RuleSelect({
  label,
  value,
  options,
  issue,
  onChange,
}: {
  label: string;
  value: string;
  options: string[];
  issue?: ValidationIssue;
  onChange: (value: string) => void;
}) {
  return (
    <label className={["rule-datum", issue ? "invalid" : ""].join(" ")}>
      <span>{label}</span>
      <select className={issue ? "invalid" : ""} value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
      {issue && <small>{issue.message}</small>}
    </label>
  );
}

function NestedParameterPreview({
  value,
  schema,
}: {
  value: JsonValue;
  schema: SchemaSpec;
}) {
  if (schema.kind === "array") {
    const items = Array.isArray(value) ? value : [];

    return (
      <div className="nested-preview">
        {items.length === 0 ? (
          <p className="empty-state">Empty array.</p>
        ) : (
          items.map((item, index) => (
            <NestedItem key={index} title={`Item ${index + 1}`} value={item} schema={schema.item} />
          ))
        )}
      </div>
    );
  }

  if (schema.kind === "object") {
    return <NestedObject value={value} fields={schema.fields} />;
  }

  if (schema.kind === "tagged_union") {
    return <NestedTaggedUnion value={value} schema={schema} />;
  }

  return <pre>{formatJsonValue(value)}</pre>;
}

function NestedItem({
  title,
  value,
  schema,
}: {
  title: string;
  value: JsonValue;
  schema: SchemaSpec;
}) {
  return (
    <div className="nested-item">
      <div className="nested-item-title">
        <span>{title}</span>
        <strong>{summarizeNestedValue(value, schema)}</strong>
      </div>
      <NestedParameterPreview value={value} schema={schema} />
    </div>
  );
}

function NestedObject({ value, fields }: { value: JsonValue; fields: FieldSpec[] }) {
  const objectValue = isJsonObject(value) ? value : {};

  return (
    <div className="nested-object">
      {fields.map((field) => {
        const childValue = objectValue[field.key] ?? null;

        return (
          <div className="nested-field" key={field.key}>
            <span>{field.label}</span>
            {field.schema ? (
              <NestedParameterPreview value={childValue} schema={field.schema} />
            ) : (
              <strong>{formatJsonValue(childValue)}</strong>
            )}
          </div>
        );
      })}
    </div>
  );
}

function NestedTaggedUnion({
  value,
  schema,
}: {
  value: JsonValue;
  schema: Extract<SchemaSpec, { kind: "tagged_union" }>;
}) {
  const objectValue = isJsonObject(value) ? value : {};
  const tagValue = objectValue[schema.tag];
  const variant = schema.variants.find((candidate) => candidate.tag_value === tagValue);

  return (
    <div className="nested-object tagged-union">
      <div className="nested-field">
        <span>{schema.tag}</span>
        <strong>{variant?.label ?? formatJsonValue(tagValue)}</strong>
      </div>
      {(variant?.fields ?? []).map((field) => (
        <div className="nested-field" key={field.key}>
          <span>{field.label}</span>
          {field.schema ? (
            <NestedParameterPreview value={objectValue[field.key] ?? null} schema={field.schema} />
          ) : (
            <strong>{formatJsonValue(objectValue[field.key] ?? null)}</strong>
          )}
        </div>
      ))}
    </div>
  );
}

function DescriptorSummary({
  descriptor,
  configuredCount,
}: {
  descriptor: ProcessorDescriptor;
  configuredCount: number;
}) {
  return (
    <div className="descriptor-summary">
      <div>
        <strong>{descriptor.display_name}</strong>
        <span>{descriptor.category}</span>
      </div>
      <p>{descriptor.description}</p>
      <small>
        {configuredCount}/{descriptor.fields.length} parameters configured
      </small>
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
  saveState: SaveState;
  onSave: (value: string) => Promise<void>;
}) {
  const [draftValue, setDraftValue] = useState(value);
  const isDirty = draftValue !== value;

  useEffect(() => {
    setDraftValue(value);
  }, [value]);

  return (
    <div className={isDirty ? "parameter-row dirty" : "parameter-row"}>
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
      <button
        className="parameter-save"
        disabled={!isDirty || saveState === "saving"}
        onClick={() => onSave(draftValue)}
      >
        {saveState === "saving" ? "Saving" : "Save"}
      </button>
    </div>
  );
}

function UnsavedChangesDialog({
  action,
  onCancel,
  onConfirm,
}: {
  action: PendingDiscardAction;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
}) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onCancel();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onCancel]);

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onCancel}>
      <section
        className="confirm-dialog neutral"
        role="dialog"
        aria-modal="true"
        aria-labelledby="unsaved-changes-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="confirm-dialog-header">
          <div className="confirm-dialog-icon">
            <AlertCircle size={20} />
          </div>
          <div>
            <span>Unsaved Draft</span>
            <h2 id="unsaved-changes-title">{action.title}</h2>
          </div>
        </div>

        <div className="confirm-dialog-body">
          <p className="confirm-dialog-message">{action.detail}</p>
        </div>

        <div className="confirm-dialog-actions">
          <button className="secondary-action" onClick={onCancel}>
            Keep Editing
          </button>
          <button className="primary-action" onClick={onConfirm}>
            <RotateCcw size={15} />
            <span>{action.confirmLabel}</span>
          </button>
        </div>
      </section>
    </div>
  );
}

function DeleteNodeDialog({
  pendingDelete,
  saveState,
  onCancel,
  onConfirm,
}: {
  pendingDelete: PendingDelete;
  saveState: SaveState;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
}) {
  const { node, impact } = pendingDelete;
  const isSaving = saveState === "saving";

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !isSaving) {
        onCancel();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isSaving, onCancel]);

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={() => !isSaving && onCancel()}>
      <section
        className="confirm-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-node-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="confirm-dialog-header">
          <div className="confirm-dialog-icon">
            <AlertTriangle size={20} />
          </div>
          <div>
            <span>Delete Node</span>
            <h2 id="delete-node-title">{node.display_name}</h2>
          </div>
        </div>

        <div className="confirm-dialog-body">
          <KeyValue label="Processor" value={node.processor_type} />
          <KeyValue label="Config path" value={node.config_path} />
          {impact.outputChannel ? (
            <div className="delete-impact">
              <span>Output cleanup</span>
              <strong>{impact.outputChannel}</strong>
              {impact.downstreamNodes.length > 0 ? (
                <>
                  <p>This channel will be removed from downstream inputs:</p>
                  <div className="impact-node-list">
                    {impact.downstreamNodes.map((downstreamNode) => (
                      <code key={downstreamNode.id}>{downstreamNode.display_name}</code>
                    ))}
                  </div>
                </>
              ) : (
                <p>No downstream consumers use this output channel.</p>
              )}
            </div>
          ) : (
            <div className="delete-impact">
              <span>Output cleanup</span>
              <p>This node does not produce a channel.</p>
            </div>
          )}
        </div>

        <div className="confirm-dialog-actions">
          <button className="secondary-action" disabled={isSaving} onClick={onCancel}>
            Cancel
          </button>
          <button className="danger-action" disabled={isSaving} onClick={onConfirm}>
            <Trash2 size={15} />
            <span>{isSaving ? "Deleting" : "Delete Node"}</span>
          </button>
        </div>
      </section>
    </div>
  );
}

function EdgeInspector({
  edge,
  graph,
  saveState,
  onSelectNode,
  onSelectChannel,
  onDisconnectEdge,
}: {
  edge: GraphEdge;
  graph: ResolvedPipelineGraph;
  saveState: SaveState;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
  onDisconnectEdge: (edge: GraphEdge) => Promise<void>;
}) {
  const sourceNode = graph.nodes.find((node) => node.id === edge.source_node_id) ?? null;
  const targetNode = graph.nodes.find((node) => node.id === edge.target_node_id) ?? null;
  const diagnostics = graph.diagnostics.filter((diagnostic) => diagnostic.channel_name === edge.channel_name);

  return (
    <div className="inspector-body">
      <InspectorTitle eyebrow="Connection" title={edge.channel_name} />
      <div className="edge-actions">
        <button
          className="danger-action"
          disabled={saveState === "saving"}
          onClick={() => onDisconnectEdge(edge)}
        >
          <Trash2 size={15} />
          <span>{saveState === "saving" ? "Disconnecting" : "Disconnect"}</span>
        </button>
      </div>
      <InspectorSection title="Endpoints" defaultOpen>
        <div className="endpoint-list">
          <button onClick={() => onSelectNode(edge.source_node_id)}>
            <span>Source</span>
            <strong>{sourceNode?.display_name ?? edge.source_node_id}</strong>
          </button>
          <button onClick={() => onSelectNode(edge.target_node_id)}>
            <span>Target</span>
            <strong>{targetNode?.display_name ?? edge.target_node_id}</strong>
          </button>
          <button onClick={() => onSelectChannel(edge.channel_name)}>
            <span>Channel</span>
            <strong>{edge.channel_name}</strong>
          </button>
        </div>
        <KeyValue label="Target input" value={String(edge.target_input_index + 1)} />
      </InspectorSection>
      <DiagnosticsList diagnostics={diagnostics} selectedDiagnostic={null} defaultOpen={diagnostics.length > 0} />
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

function RuntimeConsole({
  logs,
  state,
  filter,
  selectedNode,
  selectedChannelName,
  onFilterChange,
  onClear,
}: {
  logs: RuntimeLogEntry[];
  state: RuntimeState;
  filter: "all" | "selection";
  selectedNode: GraphNode | null;
  selectedChannelName: string | null;
  onFilterChange: (filter: "all" | "selection") => void;
  onClear: () => void;
}) {
  const selectionTokens = runtimeSelectionTokens(selectedNode, selectedChannelName);
  const filteredLogs =
    filter === "selection" && selectionTokens.length > 0
      ? logs.filter((entry) =>
          selectionTokens.some((token) => entry.line.toLowerCase().includes(token.toLowerCase())),
        )
      : logs;
  const stateLabel =
    state === "starting"
      ? "Starting"
      : state === "running"
        ? "Running"
        : state === "stopping"
          ? "Stopping"
          : state === "error"
            ? "Error"
            : "Idle";
  const selectionLabel = selectedChannelName ?? selectedNode?.display_name ?? "No selection";

  return (
    <section className="runtime-console">
      <div className="runtime-console-header">
        <div className="runtime-console-title">
          <Terminal size={15} />
          <span>Console</span>
          <strong className={`runtime-state ${state}`}>{stateLabel}</strong>
        </div>
        <div className="runtime-console-actions">
          <div className="runtime-filter" role="group" aria-label="Console filter">
            <button
              className={filter === "all" ? "active" : ""}
              onClick={() => onFilterChange("all")}
            >
              All
            </button>
            <button
              className={filter === "selection" ? "active" : ""}
              onClick={() => onFilterChange("selection")}
              disabled={selectionTokens.length === 0}
              title={selectionLabel}
            >
              Selection
            </button>
          </div>
          <button className="runtime-clear" onClick={onClear} disabled={logs.length === 0}>
            Clear
          </button>
        </div>
      </div>
      <div className="runtime-log-list">
        {filteredLogs.length === 0 ? (
          <p className="runtime-empty">
            {filter === "selection" && selectionTokens.length > 0
              ? `No console lines match ${selectionLabel}.`
              : "Run the pipeline to stream output here."}
          </p>
        ) : (
          filteredLogs.map((entry) => (
            <div className={`runtime-log-line ${entry.stream}`} key={entry.id}>
              <span className="runtime-log-stream">{entry.stream}</span>
              <code>{renderAnsiText(entry.line)}</code>
            </div>
          ))
        )}
      </div>
    </section>
  );
}

function renderAnsiText(text: string): ReactNode[] {
  const ansiPattern = /\x1b\[([0-9;]*)m/g;
  const chunks: ReactNode[] = [];
  let cursor = 0;
  let state: AnsiTextState = { color: null, bold: false, dim: false };
  let match: RegExpExecArray | null;

  while ((match = ansiPattern.exec(text)) !== null) {
    if (match.index > cursor) {
      chunks.push(renderAnsiChunk(text.slice(cursor, match.index), state, chunks.length));
    }

    state = applyAnsiCodes(state, match[1]);
    cursor = match.index + match[0].length;
  }

  if (cursor < text.length) {
    chunks.push(renderAnsiChunk(text.slice(cursor), state, chunks.length));
  }

  return chunks;
}

function renderAnsiChunk(text: string, state: AnsiTextState, key: number) {
  if (!text) {
    return null;
  }

  const classes = [
    "ansi-text",
    state.color ? `ansi-${state.color}` : "",
    state.bold ? "ansi-bold" : "",
    state.dim ? "ansi-dim" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <span className={classes || undefined} key={key}>
      {text}
    </span>
  );
}

function applyAnsiCodes(state: AnsiTextState, rawCodes: string): AnsiTextState {
  const codes = rawCodes.length === 0 ? [0] : rawCodes.split(";").map((code) => Number(code));
  let next = { ...state };

  for (const code of codes) {
    switch (code) {
      case 0:
        next = { color: null, bold: false, dim: false };
        break;
      case 1:
        next.bold = true;
        next.dim = false;
        break;
      case 2:
        next.dim = true;
        next.bold = false;
        break;
      case 22:
        next.bold = false;
        next.dim = false;
        break;
      case 30:
      case 90:
        next.color = "black";
        break;
      case 31:
      case 91:
        next.color = "red";
        break;
      case 32:
      case 92:
        next.color = "green";
        break;
      case 33:
      case 93:
        next.color = "yellow";
        break;
      case 34:
      case 94:
        next.color = "blue";
        break;
      case 35:
      case 95:
        next.color = "magenta";
        break;
      case 36:
      case 96:
        next.color = "cyan";
        break;
      case 37:
      case 97:
        next.color = "white";
        break;
      case 39:
        next.color = null;
        break;
      default:
        break;
    }
  }

  return next;
}

function GraphCanvas({
  graph,
  configPath,
  selectedNodeId,
  selectedChannelName,
  selectedEdgeId,
  selectedDiagnosticKey,
  filterText,
  onSelectNode,
  onSelectChannel,
  onSelectEdge,
  onConnectNodes,
  onDisconnectEdge,
  onDeleteNode,
  onStartRuntime,
  onStopRuntime,
  error,
  loadState,
  runtimeState,
  runtimeLogs,
  runtimeLogFilter,
  selectedRuntimeNode,
  selectedRuntimeChannelName,
  onRuntimeLogFilterChange,
  onClearRuntimeLogs,
}: {
  graph: ResolvedPipelineGraph | null;
  configPath: string;
  selectedNodeId: string | null;
  selectedChannelName: string | null;
  selectedEdgeId: string | null;
  selectedDiagnosticKey: string | null;
  filterText: string;
  onSelectNode: (id: string | null) => void;
  onSelectChannel: (channelName: string | null) => void;
  onSelectEdge: (edgeId: string | null) => void;
  onConnectNodes: (sourceNodeId: string, targetNodeId: string) => Promise<void>;
  onDisconnectEdge: (targetNodeId: string, channelName: string) => Promise<void>;
  onDeleteNode: (nodeId: string) => Promise<void>;
  onStartRuntime: () => Promise<void>;
  onStopRuntime: () => Promise<void>;
  error: string | null;
  loadState: "idle" | "loading" | "error";
  runtimeState: RuntimeState;
  runtimeLogs: RuntimeLogEntry[];
  runtimeLogFilter: "all" | "selection";
  selectedRuntimeNode: GraphNode | null;
  selectedRuntimeChannelName: string | null;
  onRuntimeLogFilterChange: (filter: "all" | "selection") => void;
  onClearRuntimeLogs: () => void;
}) {
  const [connectionSourceNodeId, setConnectionSourceNodeId] = useState<string | null>(null);
  const [layoutRevision, setLayoutRevision] = useState(0);
  const [flowRevision, setFlowRevision] = useState(0);
  const [debugExpanded, setDebugExpanded] = useState(false);
  const previousConfigPath = useRef(configPath);
  const previousFitKey = useRef<string | null>(null);
  const flowAreaRef = useRef<HTMLDivElement | null>(null);
  const reactFlow = useReactFlow<Node<FlowNodeData>, Edge>();
  const updateNodeInternals = useUpdateNodeInternals();
  const flowStoreSnapshotText = useStore((state) => {
    let nodesWithDimensions = 0;
    let nodesWithHandleBounds = 0;

    state.nodeLookup.forEach((node) => {
      const width = node.measured?.width ?? node.width ?? node.initialWidth;
      const height = node.measured?.height ?? node.height ?? node.initialHeight;
      if (width && height) {
        nodesWithDimensions += 1;
      }
      if (node.internals.handleBounds) {
        nodesWithHandleBounds += 1;
      }
    });

    return [
      state.edges.length,
      state.edgeLookup.size,
      state.nodes.length,
      Math.round(state.width),
      Math.round(state.height),
      nodesWithDimensions,
      nodesWithHandleBounds,
    ].join("|");
  });
  const flowStoreSnapshot = useMemo(() => {
    const [
      edges = "0",
      edgeLookup = "0",
      nodes = "0",
      width = "0",
      height = "0",
      nodesWithDimensions = "0",
      nodesWithHandleBounds = "0",
    ] = flowStoreSnapshotText.split("|");

    return {
      edgeLookup: Number(edgeLookup),
      edges: Number(edges),
      height: Number(height),
      nodes: Number(nodes),
      nodesWithDimensions: Number(nodesWithDimensions),
      nodesWithHandleBounds: Number(nodesWithHandleBounds),
      width: Number(width),
    };
  }, [flowStoreSnapshotText]);
  const savedLayout = useMemo(() => readStoredLayout(configPath), [configPath, layoutRevision]);
  const [nodePositions, setNodePositions] = useState<Record<string, LayoutPosition>>(savedLayout);
  const [debugSnapshot, setDebugSnapshot] = useState<FlowDebugSnapshot | null>(null);
  const [flowErrors, setFlowErrors] = useState<string[]>([]);
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
  const runtimeActivity = useMemo(
    () => runtimeActivityFromLogs(graph, runtimeLogs),
    [graph, runtimeLogs],
  );
  const graphNodeIds = useMemo(() => graph?.nodes.map((node) => node.id) ?? [], [graph]);
  const graphNodeIdsKey = graphNodeIds.join("|");

  const flowNodes = useMemo<Node<FlowNodeData>[]>(() => {
    const query = filterText.trim().toLowerCase();
    const positions = layoutNodePositions(graph?.nodes ?? [], { ...savedLayout, ...nodePositions });

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
      const connectionState = connectionSourceNodeId
        ? connectionValidationMessage(graph, connectionSourceNodeId, node.id) === null
          ? "valid"
          : "invalid"
        : null;

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
          runtimeActive: runtimeActivity.nodeIds.has(node.id),
          activeChannelNames: [...node.input_channels, node.output_channel ?? ""].filter((channelName) =>
            runtimeActivity.channelNames.has(channelName),
          ),
          connectionState,
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
    connectionSourceNodeId,
    onSelectChannel,
    nodePositions,
    runtimeActivity,
    savedLayout,
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
        selected: edge.id === selectedEdgeId,
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
          onSelectEdge,
          pathState,
          severity: severityForDiagnostics(channelDiagnostics),
          runtimeActive: runtimeActivity.channelNames.has(edge.channel_name),
        },
        animated: channel?.channel_type === "broadcast" || channel?.channel_type === "fanout",
        style: { stroke: color, strokeWidth: 2.1 },
      };
    });
  }, [channelByName, focusState, graph, onSelectEdge, runtimeActivity, selectedEdgeId]);
  const flowKey = `${configPath}:${flowRevision}`;
  const updateDebugSnapshot = useCallback(() => {
    const viewport = reactFlow.getViewport();
    const flowArea = flowAreaRef.current;
    const viewportElement = flowArea?.querySelector<HTMLElement>(".react-flow__viewport") ?? null;
    const edgeLayer = flowArea?.querySelector<HTMLElement>(".react-flow__edges") ?? null;
    const domNodes = flowArea?.querySelectorAll(".react-flow__node").length ?? 0;
    const domEdges = flowArea?.querySelectorAll(".react-flow__edge").length ?? 0;
    const sourceHandles =
      flowArea?.querySelectorAll(".react-flow__handle.source, .xyflow__handle.source").length ?? 0;
    const targetHandles =
      flowArea?.querySelectorAll(".react-flow__handle.target, .xyflow__handle.target").length ?? 0;

    setDebugSnapshot({
      appNodes: flowNodes.length,
      renderedNodes: reactFlow.getNodes().length,
      appEdges: flowEdges.length,
      renderedEdges: reactFlow.getEdges().length,
      domNodes,
      domEdges,
      sourceHandles,
      targetHandles,
      edgeLayerChildren: edgeLayer?.children.length ?? 0,
      storeEdges: flowStoreSnapshot.edges,
      storeEdgeLookup: flowStoreSnapshot.edgeLookup,
      storeNodes: flowStoreSnapshot.nodes,
      storeWidth: flowStoreSnapshot.width,
      storeHeight: flowStoreSnapshot.height,
      nodesWithDimensions: flowStoreSnapshot.nodesWithDimensions,
      nodesWithHandleBounds: flowStoreSnapshot.nodesWithHandleBounds,
      flowErrors,
      viewportTransform: viewportElement?.style.transform || "none",
      viewportX: Math.round(viewport.x),
      viewportY: Math.round(viewport.y),
      zoom: Number(viewport.zoom.toFixed(3)),
    });
  }, [flowEdges.length, flowErrors, flowNodes.length, flowStoreSnapshot, reactFlow]);

  const recordFlowError = useCallback((id: string, message: string) => {
    setFlowErrors((currentErrors) => {
      const nextError = `${id}: ${message}`;
      return [nextError, ...currentErrors.filter((error) => error !== nextError)].slice(0, 5);
    });
  }, []);

  const refreshFlowGeometry = useCallback(() => {
    if (graphNodeIds.length === 0) {
      return;
    }

    updateNodeInternals(graphNodeIds);
    window.setTimeout(updateDebugSnapshot, 80);
  }, [graphNodeIds, updateDebugSnapshot, updateNodeInternals]);

  const hasMissingEdgeDom = useCallback(() => {
    const flowArea = flowAreaRef.current;
    if (!flowArea || flowEdges.length === 0) {
      return false;
    }

    const domNodes = flowArea.querySelectorAll(".react-flow__node").length;
    const domEdges = flowArea.querySelectorAll(".react-flow__edge").length;
    return domNodes > 0 && domEdges < flowEdges.length;
  }, [flowEdges.length]);

  const repairMissingEdgeDom = useCallback(() => {
    if (hasMissingEdgeDom()) {
      refreshFlowGeometry();
    } else {
      updateDebugSnapshot();
    }
  }, [hasMissingEdgeDom, refreshFlowGeometry, updateDebugSnapshot]);

  const recoverFlowView = useCallback(() => {
    setFlowRevision((revision) => revision + 1);
    previousFitKey.current = null;
    window.requestAnimationFrame(() => {
      reactFlow.fitView({ padding: 0.18, duration: 180 });
      window.setTimeout(updateDebugSnapshot, 220);
    });
  }, [reactFlow, updateDebugSnapshot]);

  useEffect(() => {
    window.requestAnimationFrame(refreshFlowGeometry);
  }, [configPath, graphNodeIdsKey, refreshFlowGeometry]);

  useEffect(() => {
    let frame: number | null = null;
    const flowArea = flowAreaRef.current;

    const scheduleGeometryRefresh = () => {
      if (frame !== null) {
        window.cancelAnimationFrame(frame);
      }

      frame = window.requestAnimationFrame(() => {
        frame = null;
        refreshFlowGeometry();
      });
    };

    window.addEventListener("resize", scheduleGeometryRefresh);
    const resizeObserver = new ResizeObserver(scheduleGeometryRefresh);
    if (flowArea) {
      resizeObserver.observe(flowArea);
    }

    return () => {
      if (frame !== null) {
        window.cancelAnimationFrame(frame);
      }
      window.removeEventListener("resize", scheduleGeometryRefresh);
      resizeObserver.disconnect();
    };
  }, [refreshFlowGeometry]);

  useEffect(() => {
    let frame: number | null = null;

    const scheduleMissingEdgeRepair = () => {
      if (frame !== null) {
        window.cancelAnimationFrame(frame);
      }

      frame = window.requestAnimationFrame(() => {
        frame = null;
        repairMissingEdgeDom();
      });
    };

    window.addEventListener("scroll", scheduleMissingEdgeRepair, true);
    const interval = window.setInterval(repairMissingEdgeDom, 500);
    return () => {
      if (frame !== null) {
        window.cancelAnimationFrame(frame);
      }
      window.removeEventListener("scroll", scheduleMissingEdgeRepair, true);
      window.clearInterval(interval);
    };
  }, [repairMissingEdgeDom]);

  useEffect(() => {
    const toggleFlowDebug = (event: KeyboardEvent) => {
      if (!event.shiftKey || !(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "d") {
        return;
      }

      if (isTextEditingTarget(event.target)) {
        return;
      }

      event.preventDefault();
      setDebugExpanded((expanded) => !expanded);
    };

    window.addEventListener("keydown", toggleFlowDebug);
    return () => window.removeEventListener("keydown", toggleFlowDebug);
  }, []);

  const onNodesChange = useCallback(
    (changes: NodeChange<Node<FlowNodeData>>[]) => {
      const removeChanges = changes.filter((change) => change.type === "remove");
      const positionChanges = changes.filter((change) => change.type === "position" && change.position);

      removeChanges.forEach((change) => {
        onDeleteNode(change.id);
      });

      if (positionChanges.length > 0) {
        setNodePositions((currentPositions) => {
          const nextPositions = { ...currentPositions };
          positionChanges.forEach((change) => {
            if (change.type === "position" && change.position) {
              nextPositions[change.id] = {
                x: Math.round(change.position.x),
                y: Math.round(change.position.y),
              };
            }
          });
          return nextPositions;
        });
      }
    },
    [onDeleteNode],
  );
  const onEdgesChange = useCallback((_changes: EdgeChange[]) => {}, []);
  const onConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) {
        return;
      }

      onConnectNodes(connection.source, connection.target);
    },
    [onConnectNodes],
  );
  const isValidConnection = useCallback(
    (connection: Connection | Edge) =>
      Boolean(connection.source && connection.target) &&
      connectionValidationMessage(graph, connection.source ?? "", connection.target ?? "") === null,
    [graph],
  );

  useEffect(() => {
    if (previousConfigPath.current !== configPath) {
      previousConfigPath.current = configPath;
      setNodePositions(savedLayout);
      setFlowErrors([]);
    }
  }, [configPath, savedLayout]);

  useEffect(() => {
    previousFitKey.current = null;
    setFlowRevision((revision) => revision + 1);
  }, [graph]);

  useEffect(() => {
    if (!graph || flowNodes.length === 0) {
      return;
    }

    const fitKey = `${configPath}:${graph.nodes.map((node) => node.id).join("|")}`;
    if (previousFitKey.current === fitKey) {
      return;
    }

    previousFitKey.current = fitKey;
    window.requestAnimationFrame(() => {
      reactFlow.fitView({ padding: 0.18, duration: 180 });
    });
  }, [configPath, flowNodes.length, graph, reactFlow]);

  useEffect(() => {
    if (graph) {
      pruneStoredLayout(configPath, graph.nodes);
    }
  }, [configPath, graph]);

  useEffect(() => {
    updateDebugSnapshot();
    const interval = window.setInterval(updateDebugSnapshot, 500);
    return () => window.clearInterval(interval);
  }, [updateDebugSnapshot]);

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
      <div className="layout-toolbar">
        <button
          className={runtimeState === "running" ? "runtime-button running" : "runtime-button"}
          onClick={runtimeState === "running" || runtimeState === "stopping" ? onStopRuntime : onStartRuntime}
          disabled={runtimeState === "starting" || runtimeState === "stopping"}
          title={runtimeState === "running" ? "Stop pipeline" : "Run pipeline"}
          aria-label={runtimeState === "running" ? "Stop pipeline" : "Run pipeline"}
        >
          {runtimeState === "running" || runtimeState === "stopping" ? (
            <Square size={15} />
          ) : (
            <Play size={15} />
          )}
        </button>
        <button
          onClick={() => {
            clearStoredLayout(configPath);
            setNodePositions({});
            setLayoutRevision((revision) => revision + 1);
            window.requestAnimationFrame(() => {
              reactFlow.fitView({ padding: 0.18, duration: 180 });
            });
          }}
          title="Reset node layout"
          aria-label="Reset node layout"
        >
          <RotateCcw size={15} />
        </button>
        <button
          className={debugExpanded ? "debug-toolbar-button active" : "debug-toolbar-button"}
          onClick={() => setDebugExpanded((expanded) => !expanded)}
          title={debugExpanded ? "Hide flow debug (Cmd/Ctrl+Shift+D)" : "Show flow debug (Cmd/Ctrl+Shift+D)"}
          aria-label={debugExpanded ? "Hide flow debug" : "Show flow debug"}
        >
          Debug
        </button>
        <button
          className="debug-toolbar-button"
          onClick={recoverFlowView}
          title="Recover graph renderer"
          aria-label="Recover graph renderer"
        >
          Recover
        </button>
      </div>
      <RuntimeConsole
        logs={runtimeLogs}
        state={runtimeState}
        filter={runtimeLogFilter}
        selectedNode={selectedRuntimeNode}
        selectedChannelName={selectedRuntimeChannelName}
        onFilterChange={onRuntimeLogFilterChange}
        onClear={onClearRuntimeLogs}
      />
      <div className="flow-area" ref={flowAreaRef}>
        <ReactFlow
          key={flowKey}
          nodes={flowNodes}
          edges={flowEdges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onError={recordFlowError}
          onConnect={onConnect}
          isValidConnection={isValidConnection}
          onConnectStart={(_, params) => setConnectionSourceNodeId(params.nodeId ?? null)}
          onConnectEnd={() => setConnectionSourceNodeId(null)}
          onEdgesDelete={(deletedEdges) => {
            deletedEdges.forEach((edge) => {
              const channelName = (edge.data as ChannelEdgeData | undefined)?.channelName;
              if (edge.target && channelName) {
                onDisconnectEdge(edge.target, channelName);
              }
            });
          }}
          onNodeDragStop={(_, node) => {
            setNodePositions((currentPositions) => {
              const nextPositions = {
                ...currentPositions,
                [node.id]: {
                  x: Math.round(node.position.x),
                  y: Math.round(node.position.y),
                },
              };
              writeStoredNodePositions(configPath, flowNodes, nextPositions);
              return nextPositions;
            });
            setLayoutRevision((revision) => revision + 1);
          }}
          onNodeClick={(_, node) => {
            onSelectNode(node.id);
          }}
          onEdgeClick={(_, edge) => {
            onSelectEdge(edge.id);
          }}
          onPaneClick={() => {
            onSelectNode(null);
          }}
          onlyRenderVisibleElements={false}
          colorMode="dark"
          nodesDraggable
          nodesConnectable
          elementsSelectable
        >
          <Background color="#243235" gap={24} size={1} />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
      {debugExpanded && (
        <FlowDebugOverlay
          snapshot={debugSnapshot}
          configPath={configPath}
          flowKey={flowKey}
          runtimeState={runtimeState}
          onClose={() => setDebugExpanded(false)}
          onRecover={recoverFlowView}
        />
      )}
    </div>
  );
}

function FlowDebugOverlay({
  snapshot,
  configPath,
  flowKey,
  runtimeState,
  onClose,
  onRecover,
}: {
  snapshot: FlowDebugSnapshot | null;
  configPath: string;
  flowKey: string;
  runtimeState: RuntimeState;
  onClose: () => void;
  onRecover: () => void;
}) {
  return (
    <aside className="flow-debug-overlay">
      <div className="flow-debug-title">
        <button onClick={onClose}>Hide</button>
        <span>Flow Debug</span>
        <button onClick={onRecover}>Recover</button>
      </div>
      <dl>
        <div>
          <dt>App nodes</dt>
          <dd>{snapshot?.appNodes ?? "-"}</dd>
        </div>
        <div>
          <dt>Flow nodes</dt>
          <dd>{snapshot?.renderedNodes ?? "-"}</dd>
        </div>
        <div>
          <dt>App edges</dt>
          <dd>{snapshot?.appEdges ?? "-"}</dd>
        </div>
        <div>
          <dt>Flow edges</dt>
          <dd>{snapshot?.renderedEdges ?? "-"}</dd>
        </div>
        <div>
          <dt>DOM nodes</dt>
          <dd>{snapshot?.domNodes ?? "-"}</dd>
        </div>
        <div>
          <dt>DOM edges</dt>
          <dd>{snapshot?.domEdges ?? "-"}</dd>
        </div>
        <div>
          <dt>Edge layer children</dt>
          <dd>{snapshot?.edgeLayerChildren ?? "-"}</dd>
        </div>
        <div>
          <dt>Store edges</dt>
          <dd>{snapshot?.storeEdges ?? "-"}</dd>
        </div>
        <div>
          <dt>Store edge lookup</dt>
          <dd>{snapshot?.storeEdgeLookup ?? "-"}</dd>
        </div>
        <div>
          <dt>Store nodes</dt>
          <dd>{snapshot?.storeNodes ?? "-"}</dd>
        </div>
        <div>
          <dt>Store size</dt>
          <dd>
            {snapshot
              ? `${snapshot.storeWidth} x ${snapshot.storeHeight}`
              : "-"}
          </dd>
        </div>
        <div>
          <dt>Source handles</dt>
          <dd>{snapshot?.sourceHandles ?? "-"}</dd>
        </div>
        <div>
          <dt>Target handles</dt>
          <dd>{snapshot?.targetHandles ?? "-"}</dd>
        </div>
        <div>
          <dt>Measured nodes</dt>
          <dd>{snapshot?.nodesWithDimensions ?? "-"}</dd>
        </div>
        <div>
          <dt>Handle-bound nodes</dt>
          <dd>{snapshot?.nodesWithHandleBounds ?? "-"}</dd>
        </div>
        <div>
          <dt>Viewport</dt>
          <dd>
            {snapshot
              ? `${snapshot.viewportX}, ${snapshot.viewportY} @ ${snapshot.zoom}`
              : "-"}
          </dd>
        </div>
        <div>
          <dt>Transform</dt>
          <dd title={snapshot?.viewportTransform ?? ""}>{snapshot?.viewportTransform ?? "-"}</dd>
        </div>
        <div>
          <dt>Runtime</dt>
          <dd>{runtimeState}</dd>
        </div>
        <div>
          <dt>Config</dt>
          <dd title={configPath}>{configPath}</dd>
        </div>
        <div>
          <dt>Flow key</dt>
          <dd title={flowKey}>{flowKey}</dd>
        </div>
        <div>
          <dt>Flow errors</dt>
          <dd title={snapshot?.flowErrors.join("\n") ?? ""}>
            {snapshot?.flowErrors.length ? snapshot.flowErrors.join(" | ") : "none"}
          </dd>
        </div>
      </dl>
    </aside>
  );
}

function LiminalNode({ data, selected }: NodeProps<Node<FlowNodeData>>) {
  const {
    graphNode,
    diagnostics,
    channelDiagnosticCounts,
    runtimeActive,
    activeChannelNames,
    connectionState,
    selectedChannelName,
    onSelectChannel,
  } = data;
  const hasError = diagnostics.some((diagnostic) => diagnostic.severity === "error");
  const hasWarning = diagnostics.some((diagnostic) => diagnostic.severity === "warning");
  const diagnosticSeverity = severityForDiagnostics(diagnostics);
  const className = [
    "liminal-node",
    graphNode.kind,
    selected ? "selected" : "",
    connectionState === "valid" ? "connection-valid" : "",
    connectionState === "invalid" ? "connection-invalid" : "",
    runtimeActive ? "runtime-active" : "",
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
        {graphNode.kind !== "input" && (
          <Handle
            className="node-input-drop-handle"
            id="input-add"
            type="target"
            position={Position.Left}
            isConnectable
          />
        )}
        {graphNode.input_channels.map((channel, index) => (
          <button
            key={`${channel}-${index}`}
            className={channelClassName("input-channel", channel, selectedChannelName, activeChannelNames)}
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
              isConnectable
            />
            <span className="channel-text">{channel}</span>
            {(channelDiagnosticCounts[channel] ?? 0) > 0 && (
              <DiagnosticBadge count={channelDiagnosticCounts[channel]} severity="warning" compact />
            )}
          </button>
        ))}
        {outputChannel && (
          <button
            className={channelClassName("output-channel", outputChannel, selectedChannelName, activeChannelNames)}
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
              isConnectable
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
  onSelectEdge?: (edgeId: string) => void;
  pathState: "active" | "muted" | null;
  severity: DiagnosticSeverity | null;
  runtimeActive: boolean;
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
    onSelectEdge: undefined,
    pathState: null,
    severity: null,
    runtimeActive: false,
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
          edgeData.runtimeActive ? "runtime-active" : "",
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
            edgeData.runtimeActive ? "runtime-active" : "",
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
          onClick={(event) => {
            event.stopPropagation();
            edgeData.onSelectEdge?.(id);
          }}
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

function channelClassName(
  kindClass: string,
  channelName: string,
  selectedChannelName: string | null,
  activeChannelNames: string[] = [],
) {
  return [
    "channel",
    "nodrag",
    kindClass,
    channelName === selectedChannelName ? "selected-channel" : "",
    activeChannelNames.includes(channelName) ? "runtime-active" : "",
  ]
    .filter(Boolean)
    .join(" ");
}

function timingValue(node: GraphNode, key: string, fallback: string) {
  return node.timing.find((field) => field.key === key)?.value ?? fallback;
}

function editableKindForParameter(
  parameter: GraphParameter,
  fieldSpec?: FieldSpec,
): "string" | "number" | "enum" | "boolean" {
  if (fieldSpec?.kind === "enum" && fieldSpec.options.length > 0) {
    return "enum";
  }

  if (fieldSpec?.kind === "boolean") {
    return "boolean";
  }

  if (fieldSpec?.kind === "integer" || fieldSpec?.kind === "number") {
    return "number";
  }

  if (parameter.value_kind === "boolean") {
    return "boolean";
  }

  if (parameter.value_kind === "number") {
    return "number";
  }

  return "string";
}

function selectOptionsForValue(fieldSpec: FieldSpec, value: string) {
  if (!value || fieldSpec.options.includes(value)) {
    return fieldSpec.options;
  }

  return [value, ...fieldSpec.options];
}

function validateRules(
  rules: JsonValue[],
  actionSchema: Extract<SchemaSpec, { kind: "tagged_union" }> | null,
) {
  const issues: ValidationIssue[] = [];

  if (rules.length === 0) {
    issues.push({ path: "rules", message: "At least one rule is required." });
  }

  rules.forEach((rule, ruleIndex) => {
    const rulePath = `rules.${ruleIndex}`;
    const ruleObject = isJsonObject(rule) ? rule : {};
    const condition = isJsonObject(ruleObject.condition) ? ruleObject.condition : {};
    const actions = Array.isArray(ruleObject.actions) ? ruleObject.actions : [];
    const elseActions = Array.isArray(ruleObject.else_actions) ? ruleObject.else_actions : [];
    const fieldPath = condition.field_path;
    const operation = condition.operation;

    if (typeof fieldPath !== "string" || fieldPath.trim().length === 0) {
      issues.push({
        path: `${rulePath}.condition.field_path`,
        message: `Rule ${ruleIndex + 1}: field path is required.`,
      });
    }

    if (typeof operation !== "string" || !conditionOperationOptions.includes(operation)) {
      issues.push({
        path: `${rulePath}.condition.operation`,
        message: `Rule ${ruleIndex + 1}: operation is not supported.`,
      });
    }

    if (["<", "<=", ">", ">="].includes(typeof operation === "string" ? operation : "")) {
      const conditionValue = condition.value;
      if (typeof conditionValue !== "number") {
        issues.push({
          path: `${rulePath}.condition.value`,
          message: `Rule ${ruleIndex + 1}: comparison value must be numeric.`,
        });
      }
    }

    if (actions.length === 0) {
      issues.push({
        path: `${rulePath}.actions`,
        message: `Rule ${ruleIndex + 1}: at least one action is required.`,
      });
    }

    validateRuleActions(actions, `${rulePath}.actions`, `Rule ${ruleIndex + 1} action`, actionSchema, issues);
    validateRuleActions(
      elseActions,
      `${rulePath}.else_actions`,
      `Rule ${ruleIndex + 1} else action`,
      actionSchema,
      issues,
    );
  });

  return issues;
}

function validateRuleActions(
  actions: JsonValue[],
  path: string,
  label: string,
  actionSchema: Extract<SchemaSpec, { kind: "tagged_union" }> | null,
  issues: ValidationIssue[],
) {
  actions.forEach((action, actionIndex) => {
    const actionPath = `${path}.${actionIndex}`;
    const actionObject = isJsonObject(action) ? action : {};
    const type = actionObject.type;
    const variant =
      typeof type === "string"
        ? actionSchema?.variants.find((candidate) => candidate.tag_value === type)
        : undefined;

    if (typeof type !== "string" || !variant) {
      issues.push({
        path: `${actionPath}.type`,
        message: `${label} ${actionIndex + 1}: action type is not supported.`,
      });
      return;
    }

    variant.fields.forEach((field) => {
      const value = actionObject[field.key];
      if (!field.required || !isEmptyRequiredValue(value)) {
        return;
      }

      issues.push({
        path: `${actionPath}.${field.key}`,
        message: `${label} ${actionIndex + 1}: ${field.label.toLowerCase()} is required.`,
      });
    });

    if (
      type === "copy_field" &&
      typeof actionObject.source_field === "string" &&
      actionObject.source_field === actionObject.target_field
    ) {
      issues.push({
        path: `${actionPath}.target_field`,
        message: `${label} ${actionIndex + 1}: source and target must differ.`,
      });
    }

    if (
      type === "rename_field" &&
      typeof actionObject.old_field === "string" &&
      actionObject.old_field === actionObject.new_field
    ) {
      issues.push({
        path: `${actionPath}.new_field`,
        message: `${label} ${actionIndex + 1}: old and new fields must differ.`,
      });
    }
  });
}

function isEmptyRequiredValue(value: JsonValue | undefined) {
  if (value === undefined || value === null) {
    return true;
  }

  if (typeof value === "string") {
    return value.trim().length === 0;
  }

  return false;
}

function issueForPath(issues: ValidationIssue[], path: string) {
  return issues.find((issue) => issue.path === path || issue.path.endsWith(`.${path}`));
}

function isJsonObject(value: JsonValue): value is { [key: string]: JsonValue } {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function formatJsonValue(value: JsonValue) {
  if (value === null) {
    return "null";
  }

  if (typeof value === "string") {
    return value;
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  return JSON.stringify(value);
}

function parseJsonLikeValue(value: string): JsonValue {
  const trimmed = value.trim();

  if (trimmed.length === 0) {
    return "";
  }

  try {
    return JSON.parse(trimmed) as JsonValue;
  } catch {
    return value;
  }
}

function ruleActionSchema(schema: SchemaSpec): Extract<SchemaSpec, { kind: "tagged_union" }> | null {
  if (schema.kind !== "array" || schema.item.kind !== "object") {
    return null;
  }

  const actionsField = schema.item.fields.find((field) => field.key === "actions");
  if (actionsField?.schema?.kind === "array" && actionsField.schema.item.kind === "tagged_union") {
    return actionsField.schema.item;
  }

  return null;
}

function defaultActionForVariant(schema: Extract<SchemaSpec, { kind: "tagged_union" }>, tagValue: string) {
  const variant = schema.variants.find((candidate) => candidate.tag_value === tagValue);
  const action: { [key: string]: JsonValue } = { [schema.tag]: tagValue };

  variant?.fields.forEach((field) => {
    action[field.key] = defaultValueForField(field);
  });

  return action;
}

function defaultRule(actionSchema: Extract<SchemaSpec, { kind: "tagged_union" }> | null): JsonValue {
  return {
    condition: {
      field_path: "",
      operation: "equals",
      value: "",
    },
    actions: actionSchema ? [defaultActionForVariant(actionSchema, actionSchema.variants[0]?.tag_value ?? "")] : [],
    else_actions: [],
  };
}

function moveArrayItem<T>(items: T[], index: number, direction: -1 | 1) {
  const nextIndex = index + direction;

  if (nextIndex < 0 || nextIndex >= items.length) {
    return items;
  }

  const nextItems = [...items];
  const [item] = nextItems.splice(index, 1);
  nextItems.splice(nextIndex, 0, item);

  return nextItems;
}

function defaultValueForField(field: FieldSpec): JsonValue {
  if (field.default_value !== null) {
    return parseJsonLikeValue(field.default_value);
  }

  if (field.kind === "boolean") {
    return false;
  }

  if (field.kind === "integer" || field.kind === "number") {
    return 0;
  }

  if (field.kind === "array") {
    return [];
  }

  if (field.kind === "object") {
    return {};
  }

  if (field.kind === "enum") {
    return field.options[0] ?? "";
  }

  return "";
}

function setObjectValue(
  value: { [key: string]: JsonValue },
  path: string[],
  nextValue: JsonValue,
): { [key: string]: JsonValue } {
  if (path.length === 0) {
    return value;
  }

  const [head, ...tail] = path;

  if (tail.length === 0) {
    return { ...value, [head]: nextValue };
  }

  const child = isJsonObject(value[head]) ? value[head] : {};

  return {
    ...value,
    [head]: setObjectValue(child, tail, nextValue),
  };
}

function ruleSummary(condition: { [key: string]: JsonValue }, actionCount: number, elseActionCount: number) {
  const fieldPath = formatJsonValue(condition.field_path ?? "condition");
  const operation = formatJsonValue(condition.operation ?? "matches");
  const actionLabel = actionCount === 1 ? "action" : "actions";
  const elseActionLabel = elseActionCount === 1 ? "else action" : "else actions";

  return `If ${fieldPath} ${operation}, ${actionCount} ${actionLabel}, ${elseActionCount} ${elseActionLabel}`;
}

function labelFromKey(key: string) {
  return key
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function summarizeNestedValue(value: JsonValue, schema: SchemaSpec) {
  if (schema.kind === "object" && isJsonObject(value)) {
    const condition = value.condition;
    const actions = value.actions;
    const elseActions = value.else_actions;

    if (isJsonObject(condition)) {
      const fieldPath = formatJsonValue(condition.field_path ?? null);
      const operation = formatJsonValue(condition.operation ?? null);
      const actionCount = Array.isArray(actions) ? actions.length : 0;
      const elseCount = Array.isArray(elseActions) ? elseActions.length : 0;
      return ruleSummary(
        { field_path: fieldPath, operation },
        actionCount,
        elseCount,
      );
    }
  }

  if (schema.kind === "tagged_union" && isJsonObject(value)) {
    return formatJsonValue(value[schema.tag] ?? null);
  }

  if (Array.isArray(value)) {
    return `${value.length} items`;
  }

  if (isJsonObject(value)) {
    return `${Object.keys(value).length} fields`;
  }

  return formatJsonValue(value);
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
  saveState: SaveState,
  error: string | null,
  isDirty: boolean,
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
      detail: "Writing the current draft to disk.",
    };
  }

  if (saveState === "error") {
    return {
      severity: "error",
      label: "Edit Rejected",
      detail: error ?? "The parameter edit could not be saved.",
    };
  }

  if (saveState === "dirty" || isDirty) {
    return {
      severity: "warning",
      label: "Unsaved Draft",
      detail: "Edits are staged in the GUI. Save to write the TOML file.",
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

function connectionValidationMessage(
  graph: ResolvedPipelineGraph | null,
  sourceNodeId: string,
  targetNodeId: string,
) {
  if (!graph) {
    return "No graph is loaded.";
  }

  if (!sourceNodeId || !targetNodeId) {
    return "Select a source and target node.";
  }

  if (sourceNodeId === targetNodeId) {
    return "A node cannot be connected to itself.";
  }

  const sourceNode = graph.nodes.find((node) => node.id === sourceNodeId);
  const targetNode = graph.nodes.find((node) => node.id === targetNodeId);

  if (!sourceNode) {
    return `Source node '${sourceNodeId}' was not found.`;
  }

  if (!targetNode) {
    return `Target node '${targetNodeId}' was not found.`;
  }

  if (targetNode.kind === "input") {
    return "Input stages cannot consume channels.";
  }

  if (!sourceNode.output_channel) {
    return `Source node '${sourceNode.display_name}' does not produce an output channel.`;
  }

  if (targetNode.input_channels.includes(sourceNode.output_channel)) {
    return `Target node '${targetNode.display_name}' already consumes '${sourceNode.output_channel}'.`;
  }

  return null;
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
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function configParentName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 1) {
    return "";
  }

  return parts[parts.length - 2];
}

function filterConfigPaths(paths: string[], filterText: string) {
  const query = filterText.trim().toLowerCase();
  if (!query) {
    return paths;
  }

  return paths.filter((path) => path.toLowerCase().includes(query));
}

function normalizeComparablePath(path: string) {
  return path.trim().replace(/\\/g, "/").replace(/\/+/g, "/").toLowerCase();
}

function runtimeSelectionTokens(selectedNode: GraphNode | null, selectedChannelName: string | null) {
  const tokens = new Set<string>();

  if (selectedChannelName) {
    tokens.add(selectedChannelName);
  }

  if (selectedNode) {
    tokens.add(selectedNode.id);
    tokens.add(selectedNode.display_name);
    tokens.add(selectedNode.config_path);
    selectedNode.input_channels.forEach((channelName) => tokens.add(channelName));
    if (selectedNode.output_channel) {
      tokens.add(selectedNode.output_channel);
    }
  }

  return [...tokens].filter((token) => token.trim().length > 0);
}

function runtimeActivityFromLogs(graph: ResolvedPipelineGraph | null, logs: RuntimeLogEntry[]) {
  const activeNodeIds = new Set<string>();
  const activeChannelNames = new Set<string>();
  const recentLogText = logs
    .slice(-25)
    .map((entry) => entry.line.toLowerCase())
    .join("\n");

  if (!graph || recentLogText.length === 0) {
    return { nodeIds: activeNodeIds, channelNames: activeChannelNames };
  }

  graph.channels.forEach((channel) => {
    if (recentLogText.includes(channel.name.toLowerCase())) {
      activeChannelNames.add(channel.name);
    }
  });

  graph.nodes.forEach((node) => {
    const nodeTokens = runtimeSelectionTokens(node, null);
    if (nodeTokens.some((token) => recentLogText.includes(token.toLowerCase()))) {
      activeNodeIds.add(node.id);
      node.input_channels.forEach((channelName) => {
        if (recentLogText.includes(channelName.toLowerCase())) {
          activeChannelNames.add(channelName);
        }
      });
      if (node.output_channel && recentLogText.includes(node.output_channel.toLowerCase())) {
        activeChannelNames.add(node.output_channel);
      }
    }
  });

  graph.edges.forEach((edge) => {
    if (activeChannelNames.has(edge.channel_name)) {
      activeNodeIds.add(edge.source_node_id);
      activeNodeIds.add(edge.target_node_id);
    }
  });

  return { nodeIds: activeNodeIds, channelNames: activeChannelNames };
}

function isTextEditingTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable
  );
}

function deletionImpact(graph: ResolvedPipelineGraph, node: GraphNode): DeleteImpact {
  if (!node.output_channel) {
    return { outputChannel: null, downstreamNodes: [] };
  }

  const downstreamNodeIds = new Set(
    graph.edges
      .filter((edge) => edge.source_node_id === node.id && edge.channel_name === node.output_channel)
      .map((edge) => edge.target_node_id),
  );

  return {
    outputChannel: node.output_channel,
    downstreamNodes: graph.nodes.filter((candidate) => downstreamNodeIds.has(candidate.id)),
  };
}

function categoryOrder(category: ProcessorCategory) {
  return category === "input" ? 0 : category === "transform" ? 1 : category === "aggregator" ? 2 : 3;
}

function pipelineNames(graph: ResolvedPipelineGraph | null) {
  const names = new Set<string>();

  graph?.nodes.forEach((node) => {
    if (node.pipeline_name) {
      names.add(node.pipeline_name);
    }
  });

  return [...names].sort();
}

function defaultNodeName(graph: ResolvedPipelineGraph | null, descriptor: ProcessorDescriptor) {
  const baseName = sanitizeNodeName(descriptor.type_name) || "node";
  const existingNames = new Set(graph?.nodes.map((node) => node.display_name) ?? []);

  if (!existingNames.has(baseName)) {
    return baseName;
  }

  for (let index = 2; index < 1000; index += 1) {
    const candidate = `${baseName}_${index}`;
    if (!existingNames.has(candidate)) {
      return candidate;
    }
  }

  return `${baseName}_${Date.now()}`;
}

function sanitizeNodeName(value: string) {
  return value
    .trim()
    .replace(/[^A-Za-z0-9_-]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function addNodeValidationMessage(
  graph: ResolvedPipelineGraph | null,
  category: ProcessorCategory,
  nodeName: string,
  pipelineName: string,
) {
  if (!nodeName) {
    return "Enter a node name.";
  }

  if (!/^[A-Za-z0-9_-]+$/.test(nodeName)) {
    return "Use letters, numbers, underscores, or hyphens.";
  }

  if ((category === "transform" || category === "aggregator") && !pipelineName) {
    return "Enter a pipeline name.";
  }

  if (pipelineName && !/^[A-Za-z0-9_-]+$/.test(pipelineName)) {
    return "Pipeline names use the same characters as node names.";
  }

  if (graph?.nodes.some((node) => node.id === nodeIdForNewNode(category, nodeName, pipelineName))) {
    return "A node with this name already exists.";
  }

  return null;
}

function nodeIdForNewNode(category: ProcessorCategory, nodeName: string, pipelineName: string | null) {
  if (category === "input") {
    return `input:${nodeName}`;
  }

  if (category === "output") {
    return `output:${nodeName}`;
  }

  return `pipeline:${pipelineName || "default_pipeline"}.stage:${nodeName}`;
}

function readStoredInspectorWidth() {
  const storedWidth = window.localStorage.getItem(inspectorWidthStorageKey);
  const parsedWidth = storedWidth ? Number(storedWidth) : defaultInspectorWidth;

  return clampInspectorWidth(Number.isFinite(parsedWidth) ? parsedWidth : defaultInspectorWidth);
}

function clampInspectorWidth(width: number) {
  return Math.min(Math.max(Math.round(width), minInspectorWidth), maxInspectorWidth);
}

function readStoredSidebarWidth(
  storageKey: string,
  fallbackWidth: number,
  minWidth: number,
  maxWidth: number,
) {
  const storedWidth = window.localStorage.getItem(storageKey);
  const parsedWidth = storedWidth ? Number(storedWidth) : fallbackWidth;

  return clampSidebarWidth(Number.isFinite(parsedWidth) ? parsedWidth : fallbackWidth, minWidth, maxWidth);
}

function clampSidebarWidth(width: number, minWidth: number, maxWidth: number) {
  return Math.min(Math.max(Math.round(width), minWidth), maxWidth);
}

function isCompactDensityViewport() {
  const isApplePlatform = /Mac|iPhone|iPad|iPod/.test(window.navigator.platform);
  return isApplePlatform || window.innerHeight <= 920;
}

function readViewportSize(): ViewportSize {
  return {
    width: window.innerWidth,
    height: window.innerHeight,
  };
}

function readStoredRecentConfigs() {
  try {
    const storedRecentConfigs = window.localStorage.getItem(recentConfigsStorageKey);
    if (!storedRecentConfigs) {
      return [];
    }

    const parsedRecentConfigs = JSON.parse(storedRecentConfigs);
    if (!Array.isArray(parsedRecentConfigs)) {
      return [];
    }

    return parsedRecentConfigs
      .filter((path): path is string => typeof path === "string" && path.trim().length > 0)
      .slice(0, maxRecentConfigs);
  } catch {
    return [];
  }
}

function writeStoredRecentConfig(path: string) {
  const normalizedPath = path.trim();
  if (!normalizedPath) {
    return readStoredRecentConfigs();
  }

  const recentConfigs = [
    normalizedPath,
    ...readStoredRecentConfigs().filter((recentPath) => recentPath !== normalizedPath),
  ].slice(0, maxRecentConfigs);

  window.localStorage.setItem(recentConfigsStorageKey, JSON.stringify(recentConfigs));
  return recentConfigs;
}

function clearStoredRecentConfigs() {
  window.localStorage.removeItem(recentConfigsStorageKey);
  return [];
}

function readStoredWorkspacePath() {
  return window.localStorage.getItem(workspacePathStorageKey) ?? "";
}

function readStoredShowExamples() {
  return window.localStorage.getItem(showExamplesStorageKey) !== "false";
}

function readStoredCollapsedState(storageKey: string) {
  return window.localStorage.getItem(storageKey) === "true";
}

function readStoredLayout(configPath: string): Record<string, LayoutPosition> {
  try {
    const storedLayout = window.localStorage.getItem(layoutStorageKey(configPath));
    if (!storedLayout) {
      return {};
    }

    const parsedLayout = JSON.parse(storedLayout) as Record<string, LayoutPosition>;
    return Object.fromEntries(
      Object.entries(parsedLayout).filter(
        ([, position]) =>
          Number.isFinite(position?.x) &&
          Number.isFinite(position?.y),
      ),
    );
  } catch {
    return {};
  }
}

function writeStoredNodePositions(
  configPath: string,
  nodes: Node<FlowNodeData>[],
  positions: Record<string, LayoutPosition>,
) {
  const layout = Object.fromEntries(
    nodes.map((node) => [
      node.id,
      {
        x: Math.round(positions[node.id]?.x ?? node.position.x),
        y: Math.round(positions[node.id]?.y ?? node.position.y),
      },
    ]),
  );

  window.localStorage.setItem(layoutStorageKey(configPath), JSON.stringify(layout));
}

function pruneStoredLayout(configPath: string, nodes: GraphNode[]) {
  const layout = readStoredLayout(configPath);
  const nodeIds = new Set(nodes.map((node) => node.id));
  const prunedLayout = Object.fromEntries(
    Object.entries(layout).filter(([nodeId]) => nodeIds.has(nodeId)),
  );

  if (Object.keys(prunedLayout).length !== Object.keys(layout).length) {
    window.localStorage.setItem(layoutStorageKey(configPath), JSON.stringify(prunedLayout));
  }
}

function clearStoredLayout(configPath: string) {
  window.localStorage.removeItem(layoutStorageKey(configPath));
}

function layoutStorageKey(configPath: string) {
  return `${layoutStoragePrefix}${configPath}`;
}

function layoutNodePositions(nodes: GraphNode[], savedLayout: Record<string, LayoutPosition>) {
  const positions = new Map<string, { x: number; y: number }>();

  (["inputs", "pipeline_stages", "outputs"] as GraphLane[]).forEach((lane) => {
    const laneNodes = nodes
      .filter((node) => node.lane === lane)
      .sort((a, b) => a.lane_index - b.lane_index || a.id.localeCompare(b.id));
    let y = laneTop;

    laneNodes.forEach((node) => {
      positions.set(node.id, savedLayout[node.id] ?? { x: laneX[lane], y });
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
