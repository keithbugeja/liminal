import { ReactNode, useState } from "react";
import {
  FieldStatus,
  GraphDiagnostic,
  ResolvedPipelineGraph,
} from "../../types";

export function InspectorTitle({ eyebrow, title }: { eyebrow: string; title: string }) {
  return (
    <div className="inspector-title">
      <span>{eyebrow}</span>
      <h2>{title}</h2>
    </div>
  );
}

export function InspectorSection({
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

export function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="key-value">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function ChannelButtonList({
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

export function NodeButtonList({
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

export function DiagnosticsList({
  diagnostics,
  selectedDiagnostic,
  defaultOpen = false,
}: {
  diagnostics: GraphDiagnostic[];
  selectedDiagnostic: GraphDiagnostic | null;
  defaultOpen?: boolean;
}) {
  return (
    <InspectorSection
      title="Diagnostics"
      badge={String(diagnostics.length)}
      defaultOpen={defaultOpen || diagnostics.length > 0}
    >
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

export function SelectedDiagnostic({ diagnostic }: { diagnostic: GraphDiagnostic }) {
  return (
    <div className={`selected-diagnostic ${diagnostic.severity}`}>
      <span>{diagnostic.severity}</span>
      <p>{diagnostic.message}</p>
    </div>
  );
}

export function FieldStatusBadge({ status, note }: { status: FieldStatus; note?: string | null }) {
  if (status === "stable") {
    return null;
  }

  return (
    <div className={`field-status ${status}`} title={note ?? undefined}>
      <span>{status}</span>
      {note && <p>{note}</p>}
    </div>
  );
}
