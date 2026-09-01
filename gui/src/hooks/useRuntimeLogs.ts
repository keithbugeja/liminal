import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  RuntimeLogEntry,
  RuntimeLogStream,
  RuntimeState,
} from "../components/runtime-console/RuntimeConsole";

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

type RuntimeEvent = {
  id: number;
  timestamp_ms: number;
  kind: string;
  stage_id: string | null;
  processor_type: string | null;
  text: string | null;
};

const maxRuntimeLogs = 500;

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
          appendRuntimeLog(
            "system",
            formatRuntimeEvent(event.payload.event),
            event.payload.emitted_at_ms,
          );
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

  const startRuntime = useCallback(async () => {
    if (hasUnsavedDraft) {
      onError("Save or revert the current draft before running the pipeline.");
      appendRuntimeLog("system", "Run blocked: save or revert the current draft first.");
      return;
    }

    setRuntimeState("starting");
    onError(null);
    clearedAtMsRef.current = 0;
    setRuntimeLogs([]);
    appendRuntimeLog("system", `Starting pipeline from ${configPath}`);

    try {
      await invoke("start_pipeline", { path: configPath });
    } catch (caught) {
      const message = String(caught);
      setRuntimeState("error");
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
  const text = event.text ? `: ${event.text}` : "";
  return `event: ${label}${stage}${processor}${text}`;
}
