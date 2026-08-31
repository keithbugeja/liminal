import {
  FileJson,
  FilePlus,
  FolderOpen,
  History,
  Loader2,
  MoreHorizontal,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
} from "lucide-react";
import { ReactNode, useState } from "react";

type SaveState = "idle" | "dirty" | "saving" | "error";

type ConfigBrowserPanelProps = {
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
};

export function ConfigBrowserPanel({
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
}: ConfigBrowserPanelProps) {
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
          className={["example", path === activePath ? "active" : "", readOnly ? "read-only" : ""]
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
