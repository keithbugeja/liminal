import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Dispatch, SetStateAction, useCallback, useEffect, useRef, useState } from "react";
import {
  RuntimeLogEntry,
  RuntimeLogStream,
  RuntimeState,
} from "../components/runtime-console/RuntimeConsole";
import {
  RuntimeEvent,
  RuntimeMessageActivity,
  RuntimeStageState,
  RuntimeStageStates,
} from "../types";

type PipelineLogEvent = {
  stream: RuntimeLogStream;
  line: string;
  emitted_at_ms: number;
};

type PipelineStateEvent = {
  state: "idle" | "running" | "stopped" | "error";
  message: string | null;
  emitted_at_ms: number;
};

type PipelineRuntimeEventPayload = {
  event: RuntimeEvent;
  emitted_at_ms: number;
};

const maxRuntimeLogs = 500;
const runtimeActivityTtlMs = 2200;

const emptyRuntimeMessageActivity: RuntimeMessageActivity = {
  stageIds: {},
  channelNames: {},
};

export function useRuntimeLogs({
  configPath,
  hasUnsavedDraft,
  onError,
}: {
  configPath: string;
  hasUnsavedDraft: boolean;
  onError: (message: string | null) => void;
}) {
  const [runtimeState, setRuntimeState] = useState<RuntimeState>("idle");
  const [runtimeLogs, setRuntimeLogs] = useState<RuntimeLogEntry[]>([]);
  const [runtimeStageStates, setRuntimeStageStates] = useState<RuntimeStageStates>({});
  const [runtimeMessageActivity, setRuntimeMessageActivity] = useState<RuntimeMessageActivity>(
    emptyRuntimeMessageActivity,
  );
  const [runtimeLogFilter, setRuntimeLogFilter] = useState<"all" | "selection">("all");
  const clearedAtMsRef = useRef(0);

  const appendRuntimeLog = useCallback(
    (stream: RuntimeLogStream, line: string, emittedAtMs?: number) => {
      if (emittedAtMs !== undefined && emittedAtMs <= clearedAtMsRef.current) {
        return;
      }

      setRuntimeLogs((logs) =>
        [...logs, { id: Date.now() + Math.random(), stream, line }].slice(-maxRuntimeLogs),
      );
    },
    [],
  );

  useEffect(() => {
    invoke<string>("pipeline_runtime_state")
      .then((state) => setRuntimeState(state === "running" ? "running" : "idle"))
      .catch(() => setRuntimeState("idle"));
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlistenLog = listen<PipelineLogEvent>("pipeline://log", (event) => {
      if (!disposed) {
        appendRuntimeLog(
          event.payload.stream,
          event.payload.line,
          event.payload.emitted_at_ms,
        );
      }
    });
    const unlistenState = listen<PipelineStateEvent>("pipeline://state", (event) => {
      if (disposed) {
        return;
      }

      setRuntimeState(event.payload.state === "running" ? "running" : "idle");
      if (event.payload.message) {
        appendRuntimeLog("system", event.payload.message, event.payload.emitted_at_ms);
      }
    });
    const unlistenRuntimeEvent = listen<PipelineRuntimeEventPayload>(
      "pipeline://runtime-event",
      (event) => {
        if (!disposed) {
          applyRuntimeEvent(event.payload.event, setRuntimeStageStates);
          applyRuntimeMessageEvent(event.payload.event, setRuntimeMessageActivity);
          applyPipelineRuntimeEvent(event.payload.event, setRuntimeState);
          if (shouldShowRuntimeEventInConsole(event.payload.event)) {
            appendRuntimeLog(
              "system",
              formatRuntimeEvent(event.payload.event),
              event.payload.emitted_at_ms,
            );
          }
        }
      },
    );

    return () => {
      disposed = true;
      void unlistenLog.then((unlisten) => unlisten());
      void unlistenState.then((unlisten) => unlisten());
      void unlistenRuntimeEvent.then((unlisten) => unlisten());
    };
  }, [appendRuntimeLog]);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setRuntimeMessageActivity((activity) => pruneRuntimeMessageActivity(activity, Date.now()));
    }, 500);

    return () => window.clearInterval(intervalId);
  }, []);

  const startRuntime = useCallback(async () => {
    if (hasUnsavedDraft) {
      onError("Save or revert the current draft before running the pipeline.");
      appendRuntimeLog("system", "Run blocked: save or revert the current draft first.");
      return;
    }

    setRuntimeState("starting");
    setRuntimeStageStates({});
    setRuntimeMessageActivity(emptyRuntimeMessageActivity);
    onError(null);
    clearedAtMsRef.current = 0;
    setRuntimeLogs([]);
    appendRuntimeLog("system", `Starting pipeline from ${configPath}`);

    try {
      await invoke("start_pipeline", { path: configPath });
    } catch (caught) {
      const message = String(caught);
      setRuntimeState("error");
      setRuntimeStageStates({});
      setRuntimeMessageActivity(emptyRuntimeMessageActivity);
      onError(message);
      appendRuntimeLog("system", message);
    }
  }, [appendRuntimeLog, configPath, hasUnsavedDraft, onError]);

  const stopRuntime = useCallback(async () => {
    setRuntimeState("stopping");
    appendRuntimeLog("system", "Stopping pipeline...");

    try {
      await invoke("stop_pipeline");
    } catch (caught) {
      const message = String(caught);
      setRuntimeState("error");
      onError(message);
      appendRuntimeLog("system", message);
    }
  }, [appendRuntimeLog, onError]);

  return {
    runtimeState,
    runtimeLogs,
    runtimeStageStates,
    runtimeMessageActivity,
    runtimeLogFilter,
    setRuntimeLogFilter,
    startRuntime,
    stopRuntime,
    clearRuntimeLogs: () => {
      clearedAtMsRef.current = Date.now();
      setRuntimeLogs([]);
    },
  };
}

function formatRuntimeEvent(event: RuntimeEvent) {
  const label = event.kind.replace(/_/g, " ");
  const stage = event.stage_id ? ` ${event.stage_id}` : "";
  const processor = event.processor_type ? ` (${event.processor_type})` : "";
  const channel = event.channel_name ? ` [${event.channel_name}]` : "";
  const text = event.text ? `: ${event.text}` : "";
  return `event: ${label}${stage}${processor}${channel}${text}`;
}

function shouldShowRuntimeEventInConsole(event: RuntimeEvent) {
  return event.kind !== "message_received" && event.kind !== "message_emitted";
}

function applyRuntimeEvent(
  event: RuntimeEvent,
  setRuntimeStageStates: Dispatch<SetStateAction<RuntimeStageStates>>,
) {
  if (event.kind === "pipeline_starting") {
    setRuntimeStageStates({});
    return;
  }

  const stageId = event.stage_id;
  if (!stageId) {
    return;
  }

  const state = runtimeStageStateForEvent(event);
  if (!state) {
    return;
  }

  setRuntimeStageStates((currentStates) => {
    const currentState = currentStates[stageId];
    const nextState =
      state === "stopped" && currentState?.state === "error" ? currentState.state : state;

    return {
      ...currentStates,
      [stageId]: {
        stageId,
        processorType: event.processor_type,
        state: nextState,
        message:
          event.kind === "processor_error"
            ? event.text
            : nextState === "error"
              ? currentState?.message ?? null
              : null,
        updatedAtMs: event.timestamp_ms,
      },
    };
  });
}

function applyRuntimeMessageEvent(
  event: RuntimeEvent,
  setRuntimeMessageActivity: Dispatch<SetStateAction<RuntimeMessageActivity>>,
) {
  if (event.kind === "pipeline_starting") {
    setRuntimeMessageActivity(emptyRuntimeMessageActivity);
    return;
  }

  if (event.kind !== "message_received" && event.kind !== "message_emitted") {
    return;
  }

  setRuntimeMessageActivity((currentActivity) =>
    pruneRuntimeMessageActivity(
      {
        stageIds: event.stage_id
          ? { ...currentActivity.stageIds, [event.stage_id]: event.timestamp_ms }
          : currentActivity.stageIds,
        channelNames: event.channel_name
          ? { ...currentActivity.channelNames, [event.channel_name]: event.timestamp_ms }
          : currentActivity.channelNames,
      },
      event.timestamp_ms,
    ),
  );
}

function pruneRuntimeMessageActivity(
  activity: RuntimeMessageActivity,
  nowMs: number,
): RuntimeMessageActivity {
  const cutoff = nowMs - runtimeActivityTtlMs;
  return {
    stageIds: Object.fromEntries(
      Object.entries(activity.stageIds).filter(([, timestampMs]) => timestampMs >= cutoff),
    ),
    channelNames: Object.fromEntries(
      Object.entries(activity.channelNames).filter(([, timestampMs]) => timestampMs >= cutoff),
    ),
  };
}

function runtimeStageStateForEvent(event: RuntimeEvent): RuntimeStageState | null {
  switch (event.kind) {
    case "stage_starting":
      return "starting";
    case "stage_running":
      return "running";
    case "stage_stopped":
      return "stopped";
    case "processor_error":
      return "error";
    default:
      return null;
  }
}

function applyPipelineRuntimeEvent(
  event: RuntimeEvent,
  setRuntimeState: Dispatch<SetStateAction<RuntimeState>>,
) {
  switch (event.kind) {
    case "pipeline_starting":
      setRuntimeState("starting");
      break;
    case "pipeline_started":
      setRuntimeState("running");
      break;
    case "pipeline_stopped":
      setRuntimeState("idle");
      break;
    default:
      break;
  }
}
