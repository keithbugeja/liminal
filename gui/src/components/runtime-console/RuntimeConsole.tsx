import { Terminal } from "lucide-react";
import {
  KeyboardEvent as ReactKeyboardEvent,
  ReactNode,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

export type RuntimeState = "idle" | "starting" | "running" | "stopping" | "error";
export type RuntimeLogStream = "stdout" | "stderr" | "system";

export type RuntimeLogEntry = {
  id: number;
  stream: RuntimeLogStream;
  line: string;
  timestampMs?: number;
};

export type RuntimeContentFilter = {
  logs: boolean;
  telemetry: boolean;
};

type RuntimeConsoleProps = {
  logs: RuntimeLogEntry[];
  state: RuntimeState;
  filter: "all" | "selection";
  contentFilter: RuntimeContentFilter;
  selectionTokens: string[];
  telemetryLogs: RuntimeLogEntry[];
  selectionEventLogs: RuntimeLogEntry[];
  selectionLabel: string;
  onFilterChange: (filter: "all" | "selection") => void;
  onContentFilterChange: (filter: RuntimeContentFilter) => void;
  onClear: () => void;
};

type AnsiTextState = {
  color: string | null;
  bold: boolean;
  dim: boolean;
};

export function RuntimeConsole({
  logs,
  state,
  filter,
  contentFilter,
  selectionTokens,
  telemetryLogs,
  selectionEventLogs,
  selectionLabel,
  onFilterChange,
  onContentFilterChange,
  onClear,
}: RuntimeConsoleProps) {
  const logListRef = useRef<HTMLDivElement | null>(null);
  const autoScrollingRef = useRef(false);
  const userScrollIntentRef = useRef(false);
  const [followLogs, setFollowLogs] = useState(true);
  const filteredLogs =
    filter === "selection" && selectionTokens.length > 0
      ? mergeRuntimeLogStreams(
          contentFilter.logs
            ? logs.filter((entry) =>
                selectionTokens.some((token) =>
                  entry.line.toLowerCase().includes(token.toLowerCase()),
                ),
              )
            : [],
          contentFilter.telemetry ? selectionEventLogs : [],
        )
      : mergeRuntimeLogStreams(
          contentFilter.logs ? logs : [],
          contentFilter.telemetry ? telemetryLogs : [],
        );
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

  useLayoutEffect(() => {
    if (!followLogs) {
      return;
    }

    const logList = logListRef.current;
    if (!logList) {
      return;
    }

    autoScrollingRef.current = true;
    requestAnimationFrame(() => {
      logList.scrollTop = logList.scrollHeight;
      requestAnimationFrame(() => {
        autoScrollingRef.current = false;
      });
    });
  }, [filteredLogs.length, filter, followLogs]);

  const markUserScrollIntent = useCallback(() => {
    userScrollIntentRef.current = true;
  }, []);

  const handleLogKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (
        followLogs &&
        ["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "].includes(event.key)
      ) {
        markUserScrollIntent();
      }
    },
    [followLogs, markUserScrollIntent],
  );

  const handleLogScroll = useCallback(() => {
    const logList = logListRef.current;
    if (!logList || !followLogs || autoScrollingRef.current || !userScrollIntentRef.current) {
      return;
    }

    const distanceFromBottom = logList.scrollHeight - logList.scrollTop - logList.clientHeight;
    if (distanceFromBottom > 16) {
      setFollowLogs(false);
    }
    userScrollIntentRef.current = false;
  }, [followLogs]);

  const resumeFollowingLogs = useCallback(() => {
    setFollowLogs(true);
    userScrollIntentRef.current = false;
    requestAnimationFrame(() => {
      const logList = logListRef.current;
      if (logList) {
        autoScrollingRef.current = true;
        logList.scrollTop = logList.scrollHeight;
        requestAnimationFrame(() => {
          autoScrollingRef.current = false;
        });
      }
    });
  }, []);
  const toggleContentFilter = useCallback(
    (key: keyof RuntimeContentFilter) => {
      const nextFilter = { ...contentFilter, [key]: !contentFilter[key] };
      if (!nextFilter.logs && !nextFilter.telemetry) {
        return;
      }
      onContentFilterChange(nextFilter);
    },
    [contentFilter, onContentFilterChange],
  );

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
          <div className="runtime-filter" role="group" aria-label="Console streams">
            <button
              className={contentFilter.logs ? "active" : ""}
              onClick={() => toggleContentFilter("logs")}
              title={contentFilter.logs ? "Hide logs" : "Show logs"}
            >
              Logs
            </button>
            <button
              className={contentFilter.telemetry ? "active" : ""}
              onClick={() => toggleContentFilter("telemetry")}
              title={contentFilter.telemetry ? "Hide telemetry" : "Show telemetry"}
            >
              Telemetry
            </button>
          </div>
          <button
            className={followLogs ? "runtime-tail active" : "runtime-tail"}
            onClick={followLogs ? () => setFollowLogs(false) : resumeFollowingLogs}
            disabled={filteredLogs.length === 0}
            title={followLogs ? "Stop following latest logs" : "Follow latest logs"}
          >
            Tail
          </button>
          <button
            className="runtime-clear"
            onClick={onClear}
            disabled={logs.length === 0 && telemetryLogs.length === 0}
          >
            Clear
          </button>
        </div>
      </div>
      <div
        className="runtime-log-list"
        ref={logListRef}
        onKeyDown={handleLogKeyDown}
        onPointerDown={markUserScrollIntent}
        onScroll={handleLogScroll}
        onTouchStart={markUserScrollIntent}
        onWheel={markUserScrollIntent}
        tabIndex={0}
      >
        {filteredLogs.length === 0 ? (
          <p className="runtime-empty">
            {filter === "selection" && selectionTokens.length > 0
              ? `No runtime lines match ${selectionLabel}.`
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

function mergeRuntimeLogStreams(logs: RuntimeLogEntry[], telemetry: RuntimeLogEntry[]) {
  return [...logs, ...telemetry].sort(
    (left, right) => (left.timestampMs ?? left.id) - (right.timestampMs ?? right.id),
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
