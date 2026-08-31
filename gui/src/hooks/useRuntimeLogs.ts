import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import {
  RuntimeLogEntry,
  RuntimeLogStream,
  RuntimeState,
} from "../components/runtime-console/RuntimeConsole";

type PipelineLogEvent = {
  stream: RuntimeLogStream;
  line: string;
};

type PipelineStateEvent = {
  state: "idle" | "running" | "stopped" | "error";
  message: string | null;
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

  const appendRuntimeLog = useCallback((stream: RuntimeLogStream, line: string) => {
    setRuntimeLogs((logs) =>
      [...logs, { id: Date.now() + Math.random(), stream, line }].slice(-maxRuntimeLogs),
    );
  }, []);

  useEffect(() => {
    invoke<string>("pipeline_runtime_state")
      .then((state) => setRuntimeState(state === "running" ? "running" : "idle"))
      .catch(() => setRuntimeState("idle"));
  }, []);

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
    if (hasUnsavedDraft) {
      onError("Save or revert the current draft before running the pipeline.");
      appendRuntimeLog("system", "Run blocked: save or revert the current draft first.");
      return;
    }

    setRuntimeState("starting");
    onError(null);
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
    clearRuntimeLogs: () => setRuntimeLogs([]),
  };
}
