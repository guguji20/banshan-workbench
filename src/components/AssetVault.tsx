import {
  AlertCircle,
  File,
  FileAudio,
  FileImage,
  FileVideo,
  FolderOpen,
  Grid2X2,
  HardDriveUpload,
  ImageOff,
  List,
  LoaderCircle,
  PackageOpen,
  RotateCcw,
  X,
  type LucideIcon,
} from "lucide-react";
import type { AssetKind } from "../generated/bsaigc/AssetKind";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { AssetSourceSelection } from "../generated/bsaigc/AssetSourceSelection";
import "./AssetVault.css";

export type AssetVaultViewMode = "list" | "grid";
export type AssetProjectFilter = "all" | "unassigned" | string;

export interface AssetProjectOption {
  id: string;
  name: string;
}

export interface AssetVaultProps {
  assets: readonly AssetRecord[];
  projects: readonly AssetProjectOption[];
  projectFilter: AssetProjectFilter;
  viewMode: AssetVaultViewMode;
  selectedSource: AssetSourceSelection | null;
  importProjectId: string | null;
  isLoading?: boolean;
  isSelectingSource?: boolean;
  isImporting?: boolean;
  error?: string | null;
  onProjectFilterChange: (projectId: AssetProjectFilter) => void;
  onViewModeChange: (mode: AssetVaultViewMode) => void;
  onChooseSource: () => void;
  onClearSource: () => void;
  onImportProjectChange: (projectId: string | null) => void;
  onImport: (source: AssetSourceSelection, projectId: string | null) => void;
  onReload?: () => void;
}

const KIND_META: Record<
  AssetKind,
  { label: string; icon: LucideIcon }
> = {
  image: { label: "图片", icon: FileImage },
  video: { label: "视频", icon: FileVideo },
  audio: { label: "音频", icon: FileAudio },
  document: { label: "文档", icon: File },
  other: { label: "其他", icon: PackageOpen },
};

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "--";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1024;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const precision = size >= 10 ? 1 : 2;
  return `${size.toFixed(precision)} ${units[unitIndex]}`;
}

function shortHash(hash: string): string {
  return hash.length > 12 ? `${hash.slice(0, 12)}...` : hash;
}

function assetProjectName(
  asset: AssetRecord,
  projects: readonly AssetProjectOption[],
): string {
  if (!asset.projectId) return "未关联项目";
  return (
    projects.find((project) => project.id === asset.projectId)?.name ??
    `项目 ${asset.projectId.slice(0, 8)}`
  );
}

function AssetPreview({ asset }: { asset: AssetRecord }) {
  const Icon = KIND_META[asset.kind].icon;
  return (
    <div
      className={`asset-vault__preview asset-vault__preview--${asset.kind}`}
      aria-label={asset.previewAvailable ? "预览已就绪" : "暂无预览"}
    >
      <Icon size={24} strokeWidth={1.6} aria-hidden="true" />
      <span>
        {asset.previewAvailable ? "预览已就绪" : "原件已入库"}
      </span>
      {!asset.previewAvailable && (
        <ImageOff size={12} className="asset-vault__preview-state" aria-hidden="true" />
      )}
    </div>
  );
}

export function AssetVault({
  assets,
  projects,
  projectFilter,
  viewMode,
  selectedSource,
  importProjectId,
  isLoading = false,
  isSelectingSource = false,
  isImporting = false,
  error = null,
  onProjectFilterChange,
  onViewModeChange,
  onChooseSource,
  onClearSource,
  onImportProjectChange,
  onImport,
  onReload,
}: AssetVaultProps) {
  const filteredAssets = assets.filter((asset) => {
    if (projectFilter === "all") return true;
    if (projectFilter === "unassigned") return asset.projectId === null;
    return asset.projectId === projectFilter;
  });

  return (
    <section className="asset-vault" aria-label="资产归档">
      <div className="asset-vault__toolbar" role="toolbar" aria-label="资产归档工具">
        <button
          type="button"
          className="asset-vault__choose-button"
          onClick={onChooseSource}
          disabled={isSelectingSource || isImporting}
        >
          {isSelectingSource ? (
            <LoaderCircle size={15} className="asset-vault__spin" />
          ) : (
            <HardDriveUpload size={15} />
          )}
          <span>{isSelectingSource ? "正在选择" : "导入资产"}</span>
        </button>

        <label className="asset-vault__filter">
          <span>项目</span>
          <select
            value={projectFilter}
            onChange={(event) =>
              onProjectFilterChange(event.currentTarget.value)
            }
          >
            <option value="all">全部项目</option>
            <option value="unassigned">未关联项目</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </label>

        <span className="asset-vault__result-count">
          {filteredAssets.length} 项资产
        </span>

        <div className="asset-vault__view-switch" aria-label="资产显示方式">
          <button
            type="button"
            className={viewMode === "list" ? "is-active" : undefined}
            aria-pressed={viewMode === "list"}
            onClick={() => onViewModeChange("list")}
            title="列表视图"
            aria-label="列表视图"
          >
            <List size={15} />
          </button>
          <button
            type="button"
            className={viewMode === "grid" ? "is-active" : undefined}
            aria-pressed={viewMode === "grid"}
            onClick={() => onViewModeChange("grid")}
            title="网格视图"
            aria-label="网格视图"
          >
            <Grid2X2 size={14} />
          </button>
        </div>

        {onReload && (
          <button
            type="button"
            className="asset-vault__reload"
            onClick={onReload}
            disabled={isLoading}
            title="刷新资产"
            aria-label="刷新资产"
          >
            <RotateCcw
              size={15}
              className={isLoading ? "asset-vault__spin" : undefined}
            />
          </button>
        )}
      </div>

      {selectedSource && (
        <div className="asset-vault__import-bar">
          <div className="asset-vault__source">
            <FolderOpen size={18} aria-hidden="true" />
            <div>
              <strong title={selectedSource.displayName}>
                {selectedSource.displayName}
              </strong>
              <span>
                {KIND_META[selectedSource.detectedKind].label} · {formatBytes(selectedSource.sizeBytes)}
              </span>
            </div>
          </div>
          <label className="asset-vault__import-project">
            <span>归属项目</span>
            <select
              value={importProjectId ?? ""}
              onChange={(event) =>
                onImportProjectChange(event.currentTarget.value || null)
              }
              disabled={isImporting}
            >
              <option value="">未关联项目</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>
          <div className="asset-vault__import-actions">
            <button
              type="button"
              className="asset-vault__clear-source"
              onClick={onClearSource}
              disabled={isImporting}
              title="移除待导入文件"
              aria-label="移除待导入文件"
            >
              <X size={16} />
            </button>
            <button
              type="button"
              className="asset-vault__confirm-import"
              onClick={() => onImport(selectedSource, importProjectId)}
              disabled={isImporting}
            >
              {isImporting ? (
                <LoaderCircle size={16} className="asset-vault__spin" />
              ) : (
                <HardDriveUpload size={16} />
              )}
              <span>{isImporting ? "正在导入" : "确认导入"}</span>
            </button>
          </div>
        </div>
      )}

      {error && (
        <div className="asset-vault__error" role="alert">
          <AlertCircle size={17} aria-hidden="true" />
          <span>{error}</span>
          {onReload && (
            <button type="button" onClick={onReload} disabled={isLoading}>
              重试加载
            </button>
          )}
        </div>
      )}


      <div className="asset-vault__content" aria-busy={isLoading}>
        {isLoading && assets.length === 0 ? (
          <div className="asset-vault__state">
            <LoaderCircle size={22} className="asset-vault__spin" aria-hidden="true" />
            <span>正在读取本地资产…</span>
          </div>
        ) : filteredAssets.length === 0 ? (
          <div className="asset-vault__state">
            <PackageOpen size={23} aria-hidden="true" />
            <span>
              {assets.length === 0
                ? "点击上方“导入资产”开始整理项目文件"
                : "当前筛选没有资产，可切换项目或导入新文件"}
            </span>
          </div>
        ) : viewMode === "list" ? (
          <div className="asset-vault__list">
            <div className="asset-vault__list-head" aria-hidden="true">
              <span>资产</span>
              <span>归属项目</span>
              <span>文件信息</span>
              <span>校验</span>
            </div>
            {filteredAssets.map((asset) => {
              const meta = KIND_META[asset.kind];
              return (
                <article className="asset-vault__list-row" key={asset.id}>
                  <div className="asset-vault__asset-identity">
                    <AssetPreview asset={asset} />
                    <div>
                      <strong title={asset.originalName}>{asset.originalName}</strong>
                      <span>{meta.label}</span>
                    </div>
                  </div>
                  <span
                    className="asset-vault__project-name"
                    title={assetProjectName(asset, projects)}
                  >
                    {assetProjectName(asset, projects)}
                  </span>
                  <div className="asset-vault__file-meta">
                    <span title={asset.mimeType}>{asset.mimeType}</span>
                    <span>{formatBytes(asset.sizeBytes)}</span>
                  </div>
                  <div className="asset-vault__integrity">
                    <code title={asset.sha256}>{shortHash(asset.sha256)}</code>
                    <span className={`asset-vault__status asset-vault__status--${asset.status}`}>
                      {asset.status === "ready" ? "已入库" : "失败"}
                    </span>
                  </div>
                </article>
              );
            })}
          </div>
        ) : (
          <div className="asset-vault__grid">
            {filteredAssets.map((asset) => {
              const meta = KIND_META[asset.kind];
              return (
                <article className="asset-vault__card" key={asset.id}>
                  <AssetPreview asset={asset} />
                  <div className="asset-vault__card-copy">
                    <div className="asset-vault__card-title">
                      <strong title={asset.originalName}>{asset.originalName}</strong>
                      <span>{meta.label}</span>
                    </div>
                    <span
                      className="asset-vault__project-name"
                      title={assetProjectName(asset, projects)}
                    >
                      {assetProjectName(asset, projects)}
                    </span>
                    <div className="asset-vault__card-meta">
                      <span>{formatBytes(asset.sizeBytes)}</span>
                      <code title={asset.sha256}>{shortHash(asset.sha256)}</code>
                    </div>
                    <span className={`asset-vault__status asset-vault__status--${asset.status}`}>
                      {asset.status === "ready" ? "已入库" : "失败"}
                    </span>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

