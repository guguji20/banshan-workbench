import { useState, type FormEvent } from "react";
import {
  AlertCircle,
  Archive,
  File,
  FileAudio,
  FileImage,
  FileVideo,
  Film,
  Grid2X2,
  List,
  LoaderCircle,
  PackageOpen,
  Pencil,
  Plus,
  RotateCcw,
  Save,
  Search,
  SlidersHorizontal,
  Sparkles,
  Star,
  UserRound,
  X,
  type LucideIcon,
} from "lucide-react";
import type { AssetRecord } from "../generated/bsaigc/AssetRecord";
import type { CaseRecord } from "../generated/bsaigc/CaseRecord";
import "./CaseLibrary.css";

export type CaseLibraryViewMode = "list" | "grid";
export type CaseLibraryBooleanFilter = "all" | "yes" | "no";

export interface CaseLibraryFilters {
  search: string;
  clientName: string;
  contentType: "all" | CaseRecord["contentType"];
  presentation: "all" | CaseRecord["presentation"];
  hasActors: CaseLibraryBooleanFilter;
  isAigc: CaseLibraryBooleanFilter;
  qualityTier: "all" | CaseRecord["qualityTier"];
}

export interface CaseEditorDraft {
  assetId: string;
  title: string;
  clientName: string;
  contentType: CaseRecord["contentType"];
  presentation: CaseRecord["presentation"];
  hasActors: boolean;
  isAigc: boolean;
  qualityTier: CaseRecord["qualityTier"];
  tags: string;
  notes: string;
}

export interface CaseEditorState {
  mode: "create" | "edit";
  caseId: string | null;
  draft: CaseEditorDraft;
}

export interface CaseLibraryProps {
  cases: readonly CaseRecord[];
  assets: readonly AssetRecord[];
  filters: CaseLibraryFilters;
  viewMode: CaseLibraryViewMode;
  editor: CaseEditorState | null;
  isLoading?: boolean;
  isSaving?: boolean;
  error?: string | null;
  onFiltersChange: (filters: CaseLibraryFilters) => void;
  onViewModeChange: (mode: CaseLibraryViewMode) => void;
  onOpenCreate: () => void;
  onOpenEdit: (caseRecord: CaseRecord) => void;
  onEditorChange: (editor: CaseEditorState) => void;
  onCloseEditor: () => void;
  onSave: (editor: CaseEditorState) => void;
  onReload?: () => void;
}

const CONTENT_TYPES: ReadonlyArray<{
  value: CaseRecord["contentType"];
  label: string;
}> = [
  { value: "brand", label: "品牌片" },
  { value: "property", label: "地产空间" },
  { value: "interview", label: "人物采访" },
  { value: "lifestyle", label: "生活美学" },
  { value: "product", label: "产品内容" },
  { value: "event", label: "活动纪实" },
  { value: "documentary", label: "纪录内容" },
  { value: "narrative", label: "剧情内容" },
  { value: "other", label: "其他" },
];

const PRESENTATIONS: ReadonlyArray<{
  value: CaseRecord["presentation"];
  label: string;
}> = [
  { value: "liveAction", label: "实拍" },
  { value: "animation", label: "动画" },
  { value: "mixedMedia", label: "混合媒介" },
  { value: "aigc", label: "AIGC" },
  { value: "graphic", label: "平面视觉" },
  { value: "other", label: "其他" },
];

const QUALITY_TIERS: ReadonlyArray<{
  value: CaseRecord["qualityTier"];
  label: string;
}> = [
  { value: "reference", label: "参考" },
  { value: "featured", label: "精选" },
  { value: "premium", label: "高端" },
];

const ASSET_KIND: Record<
  AssetRecord["kind"],
  { label: string; icon: LucideIcon }
> = {
  image: { label: "图片", icon: FileImage },
  video: { label: "视频", icon: FileVideo },
  audio: { label: "音频", icon: FileAudio },
  document: { label: "文档", icon: File },
  other: { label: "其他", icon: PackageOpen },
};

const EMPTY_FILTERS: CaseLibraryFilters = {
  search: "",
  clientName: "all",
  contentType: "all",
  presentation: "all",
  hasActors: "all",
  isAigc: "all",
  qualityTier: "all",
};

function contentTypeLabel(value: CaseRecord["contentType"]): string {
  return CONTENT_TYPES.find((option) => option.value === value)?.label ?? value;
}

function presentationLabel(value: CaseRecord["presentation"]): string {
  return PRESENTATIONS.find((option) => option.value === value)?.label ?? value;
}

function qualityLabel(value: CaseRecord["qualityTier"]): string {
  return QUALITY_TIERS.find((option) => option.value === value)?.label ?? value;
}

function formatDate(value: number): string {
  const milliseconds = value < 10_000_000_000 ? value * 1000 : value;
  const date = new Date(milliseconds);
  if (Number.isNaN(date.getTime())) return "--";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(date);
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "--";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(size >= 10 ? 1 : 2)} ${units[unit]}`;
}

function matchesBoolean(value: boolean, filter: CaseLibraryBooleanFilter): boolean {
  return filter === "all" || (filter === "yes" ? value : !value);
}

function countActiveFilters(filters: CaseLibraryFilters): number {
  return Object.entries(filters).reduce((count, [key, value]) => {
    const emptyValue = EMPTY_FILTERS[key as keyof CaseLibraryFilters];
    return value === emptyValue ? count : count + 1;
  }, 0);
}

function AssetGlyph({ asset }: { asset: AssetRecord | undefined }) {
  const Icon = asset ? ASSET_KIND[asset.kind].icon : Archive;
  return (
    <div
      className={`case-library__asset-glyph case-library__asset-glyph--${asset?.kind ?? "missing"}`}
      aria-hidden="true"
    >
      <Icon size={22} strokeWidth={1.6} />
    </div>
  );
}

function CaseBadges({ caseRecord }: { caseRecord: CaseRecord }) {
  return (
    <div className="case-library__badges">
      <span className={`case-library__quality case-library__quality--${caseRecord.qualityTier}`}>
        <Star size={11} aria-hidden="true" />
        {qualityLabel(caseRecord.qualityTier)}
      </span>
      {caseRecord.hasActors && (
        <span className="case-library__flag">
          <UserRound size={11} aria-hidden="true" />
          演员
        </span>
      )}
      {caseRecord.isAigc && (
        <span className="case-library__flag case-library__flag--aigc">
          <Sparkles size={11} aria-hidden="true" />
          AIGC
        </span>
      )}
    </div>
  );
}

export function CaseLibrary({
  cases,
  assets,
  filters,
  viewMode,
  editor,
  isLoading = false,
  isSaving = false,
  error = null,
  onFiltersChange,
  onViewModeChange,
  onOpenCreate,
  onOpenEdit,
  onEditorChange,
  onCloseEditor,
  onSave,
  onReload,
}: CaseLibraryProps) {
  const [filtersExpanded, setFiltersExpanded] = useState(false);
  const assetById = new Map(assets.map((asset) => [asset.id, asset]));
  const readyAssets = assets.filter((asset) => asset.status === "ready");
  const clients = Array.from(
    new Set(cases.map((caseRecord) => caseRecord.clientName.trim()).filter(Boolean)),
  ).sort((left, right) => left.localeCompare(right, "zh-CN"));
  const normalizedSearch = filters.search.trim().toLocaleLowerCase("zh-CN");
  const filteredCases = cases.filter((caseRecord) => {
    const asset = assetById.get(caseRecord.assetId);
    const searchable = [
      caseRecord.title,
      caseRecord.clientName,
      caseRecord.notes,
      caseRecord.tags.join(" "),
      asset?.originalName ?? "",
    ]
      .join(" ")
      .toLocaleLowerCase("zh-CN");

    return (
      (!normalizedSearch || searchable.includes(normalizedSearch)) &&
      (filters.clientName === "all" ||
        caseRecord.clientName === filters.clientName) &&
      (filters.contentType === "all" ||
        caseRecord.contentType === filters.contentType) &&
      (filters.presentation === "all" ||
        caseRecord.presentation === filters.presentation) &&
      matchesBoolean(caseRecord.hasActors, filters.hasActors) &&
      matchesBoolean(caseRecord.isAigc, filters.isAigc) &&
      (filters.qualityTier === "all" ||
        caseRecord.qualityTier === filters.qualityTier)
    );
  });
  const activeFilterCount = countActiveFilters(filters);

  function updateFilter<Key extends keyof CaseLibraryFilters>(
    key: Key,
    value: CaseLibraryFilters[Key],
  ) {
    onFiltersChange({ ...filters, [key]: value });
  }

  function updateDraft<Key extends keyof CaseEditorDraft>(
    key: Key,
    value: CaseEditorDraft[Key],
  ) {
    if (!editor) return;
    onEditorChange({
      ...editor,
      draft: { ...editor.draft, [key]: value },
    });
  }

  function submitEditor(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (editor) onSave(editor);
  }

  const editorAsset = editor ? assetById.get(editor.draft.assetId) : undefined;
  const editorCanSave = Boolean(
    editor &&
      editor.draft.assetId &&
      editor.draft.title.trim() &&
      editor.draft.clientName.trim() &&
      editorAsset?.status === "ready",
  );

  return (
    <section className="case-library" aria-labelledby="case-library-title">
      <header className="case-library__header">
        <div className="case-library__heading">
          <span>创意检索</span>
          <h1 id="case-library-title">案例素材库</h1>
        </div>
        <div className="case-library__header-actions">
          {onReload && (
            <button
              type="button"
              className="case-library__icon-button"
              onClick={onReload}
              disabled={isLoading}
              title="刷新案例"
              aria-label="刷新案例"
            >
              <RotateCcw
                size={16}
                className={isLoading ? "case-library__spin" : undefined}
              />
            </button>
          )}
          <button
            type="button"
            className="case-library__primary-button"
            onClick={onOpenCreate}
            disabled={isSaving}
          >
            <Plus size={16} aria-hidden="true" />
            新建案例
          </button>
        </div>
      </header>

      {error && (
        <div className="case-library__error" role="alert">
          <AlertCircle size={16} aria-hidden="true" />
          <span>{error}</span>
          {onReload && (
            <button type="button" onClick={onReload} disabled={isLoading}>
              重新加载
            </button>
          )}
        </div>
      )}

      <div className="case-library__search-row">
        <label className="case-library__search">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            value={filters.search}
            onChange={(event) => updateFilter("search", event.currentTarget.value)}
            placeholder="搜索标题、客户、标签或素材名"
            aria-label="搜索案例"
          />
          {filters.search && (
            <button
              type="button"
              onClick={() => updateFilter("search", "")}
              title="清空搜索"
              aria-label="清空搜索"
            >
              <X size={14} />
            </button>
          )}
        </label>
        <button
          type="button"
          className={`case-library__filter-toggle${filtersExpanded ? " is-active" : ""}`}
          onClick={() => setFiltersExpanded((expanded) => !expanded)}
          aria-expanded={filtersExpanded}
          aria-controls="case-library-filters"
        >
          <SlidersHorizontal size={15} aria-hidden="true" />
          筛选
          {activeFilterCount > 0 && <span>{activeFilterCount}</span>}
        </button>
      </div>

      <div
        id="case-library-filters"
        className={`case-library__filters${filtersExpanded ? " is-expanded" : ""}`}
      >
        <label>
          <span>客户</span>
          <select
            value={filters.clientName}
            onChange={(event) => updateFilter("clientName", event.currentTarget.value)}
          >
            <option value="all">全部客户</option>
            {clients.map((client) => (
              <option key={client} value={client}>
                {client}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>内容类型</span>
          <select
            value={filters.contentType}
            onChange={(event) =>
              updateFilter(
                "contentType",
                event.currentTarget.value as CaseLibraryFilters["contentType"],
              )
            }
          >
            <option value="all">全部类型</option>
            {CONTENT_TYPES.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>表现形式</span>
          <select
            value={filters.presentation}
            onChange={(event) =>
              updateFilter(
                "presentation",
                event.currentTarget.value as CaseLibraryFilters["presentation"],
              )
            }
          >
            <option value="all">全部形式</option>
            {PRESENTATIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>演员</span>
          <select
            value={filters.hasActors}
            onChange={(event) =>
              updateFilter(
                "hasActors",
                event.currentTarget.value as CaseLibraryBooleanFilter,
              )
            }
          >
            <option value="all">不限</option>
            <option value="yes">有演员</option>
            <option value="no">无演员</option>
          </select>
        </label>
        <label>
          <span>AIGC</span>
          <select
            value={filters.isAigc}
            onChange={(event) =>
              updateFilter(
                "isAigc",
                event.currentTarget.value as CaseLibraryBooleanFilter,
              )
            }
          >
            <option value="all">不限</option>
            <option value="yes">含 AIGC</option>
            <option value="no">不含 AIGC</option>
          </select>
        </label>
        <label>
          <span>质量</span>
          <select
            value={filters.qualityTier}
            onChange={(event) =>
              updateFilter(
                "qualityTier",
                event.currentTarget.value as CaseLibraryFilters["qualityTier"],
              )
            }
          >
            <option value="all">全部等级</option>
            {QUALITY_TIERS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="case-library__reset-filters"
          onClick={() => onFiltersChange(EMPTY_FILTERS)}
          disabled={activeFilterCount === 0}
        >
          <RotateCcw size={14} aria-hidden="true" />
          重置
        </button>
      </div>

      <div className="case-library__summary-bar">
        <span>
          <strong>{filteredCases.length}</strong> 个案例
          {filteredCases.length !== cases.length && ` / 共 ${cases.length} 个`}
        </span>
        <div className="case-library__view-switch" aria-label="案例显示方式">
          <button
            type="button"
            className={viewMode === "list" ? "is-active" : undefined}
            aria-pressed={viewMode === "list"}
            onClick={() => onViewModeChange("list")}
            title="列表视图"
            aria-label="列表视图"
          >
            <List size={16} />
          </button>
          <button
            type="button"
            className={viewMode === "grid" ? "is-active" : undefined}
            aria-pressed={viewMode === "grid"}
            onClick={() => onViewModeChange("grid")}
            title="网格视图"
            aria-label="网格视图"
          >
            <Grid2X2 size={15} />
          </button>
        </div>
      </div>

      <div className={`case-library__workspace${editor ? " has-editor" : ""}`}>
        <div className="case-library__results" aria-busy={isLoading}>
          {isLoading && cases.length === 0 ? (
            <div className="case-library__state">
              <LoaderCircle size={23} className="case-library__spin" aria-hidden="true" />
              <strong>正在读取案例库</strong>
              <span>案例索引由本地 Host 提供</span>
            </div>
          ) : filteredCases.length === 0 ? (
            <div className="case-library__state">
              <Film size={24} aria-hidden="true" />
              <strong>{cases.length === 0 ? "案例库为空" : "没有匹配的案例"}</strong>
              <span>
                {cases.length === 0
                  ? "从已入库资产创建第一条案例记录"
                  : "调整搜索条件或重置筛选"}
              </span>
              {cases.length === 0 ? (
                <button type="button" onClick={onOpenCreate}>
                  <Plus size={15} aria-hidden="true" />
                  新建案例
                </button>
              ) : (
                <button type="button" onClick={() => onFiltersChange(EMPTY_FILTERS)}>
                  <RotateCcw size={14} aria-hidden="true" />
                  重置筛选
                </button>
              )}
            </div>
          ) : viewMode === "list" ? (
            <div className="case-library__list">
              <div className="case-library__list-head" aria-hidden="true">
                <span>案例 / 素材</span>
                <span>分类</span>
                <span>属性</span>
                <span>更新</span>
                <span>操作</span>
              </div>
              {filteredCases.map((caseRecord) => {
                const asset = assetById.get(caseRecord.assetId);
                return (
                  <article className="case-library__list-row" key={caseRecord.id}>
                    <div className="case-library__identity">
                      <AssetGlyph asset={asset} />
                      <div>
                        <strong title={caseRecord.title}>{caseRecord.title}</strong>
                        <span title={caseRecord.clientName}>{caseRecord.clientName}</span>
                        <small title={asset?.originalName ?? "素材记录不可用"}>
                          {asset?.originalName ?? "素材记录不可用"}
                        </small>
                      </div>
                    </div>
                    <div className="case-library__classification">
                      <strong>{contentTypeLabel(caseRecord.contentType)}</strong>
                      <span>{presentationLabel(caseRecord.presentation)}</span>
                    </div>
                    <div className="case-library__row-attributes">
                      <CaseBadges caseRecord={caseRecord} />
                      <div className="case-library__tags">
                        {caseRecord.tags.slice(0, 2).map((tag) => (
                          <span key={tag}>{tag}</span>
                        ))}
                        {caseRecord.tags.length > 2 && (
                          <span>+{caseRecord.tags.length - 2}</span>
                        )}
                      </div>
                    </div>
                    <div className="case-library__updated">
                      <strong>{formatDate(caseRecord.updatedAt)}</strong>
                      <span>R{caseRecord.revision}</span>
                    </div>
                    <button
                      type="button"
                      className="case-library__edit-button"
                      onClick={() => onOpenEdit(caseRecord)}
                      title={`编辑 ${caseRecord.title}`}
                      aria-label={`编辑 ${caseRecord.title}`}
                    >
                      <Pencil size={15} />
                    </button>
                  </article>
                );
              })}
            </div>
          ) : (
            <div className="case-library__grid">
              {filteredCases.map((caseRecord) => {
                const asset = assetById.get(caseRecord.assetId);
                return (
                  <article className="case-library__card" key={caseRecord.id}>
                    <div className="case-library__card-preview">
                      <AssetGlyph asset={asset} />
                      <span>{asset ? ASSET_KIND[asset.kind].label : "素材不可用"}</span>
                      <button
                        type="button"
                        onClick={() => onOpenEdit(caseRecord)}
                        title={`编辑 ${caseRecord.title}`}
                        aria-label={`编辑 ${caseRecord.title}`}
                      >
                        <Pencil size={14} />
                      </button>
                    </div>
                    <div className="case-library__card-body">
                      <div className="case-library__card-title">
                        <strong title={caseRecord.title}>{caseRecord.title}</strong>
                        <span title={caseRecord.clientName}>{caseRecord.clientName}</span>
                      </div>
                      <div className="case-library__card-classification">
                        <span>{contentTypeLabel(caseRecord.contentType)}</span>
                        <span>{presentationLabel(caseRecord.presentation)}</span>
                      </div>
                      <CaseBadges caseRecord={caseRecord} />
                      <div className="case-library__tags">
                        {caseRecord.tags.slice(0, 3).map((tag) => (
                          <span key={tag}>{tag}</span>
                        ))}
                        {caseRecord.tags.length > 3 && (
                          <span>+{caseRecord.tags.length - 3}</span>
                        )}
                      </div>
                      <div className="case-library__card-footer">
                        <span title={asset?.originalName ?? "素材记录不可用"}>
                          {asset?.originalName ?? "素材记录不可用"}
                        </span>
                        <time>{formatDate(caseRecord.updatedAt)}</time>
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </div>

        {editor && (
          <aside className="case-library__editor" aria-labelledby="case-editor-title">
            <form onSubmit={submitEditor}>
              <header className="case-library__editor-header">
                <div>
                  <span>{editor.mode === "create" ? "创建记录" : "编辑记录"}</span>
                  <h2 id="case-editor-title">
                    {editor.mode === "create" ? "新建案例" : "编辑案例"}
                  </h2>
                </div>
                <button
                  type="button"
                  onClick={onCloseEditor}
                  disabled={isSaving}
                  title="关闭编辑面板"
                  aria-label="关闭编辑面板"
                >
                  <X size={17} />
                </button>
              </header>

              <div className="case-library__editor-fields">
                <label className="case-library__field case-library__field--wide">
                  <span>
                    来源资产 <em>必填</em>
                    {editor.mode === "edit" && (
                      <small className="case-library__field-note">
                        （编辑模式下不可更换；如需换素材请新建案例）
                      </small>
                    )}
                  </span>
                  <select
                    value={editor.draft.assetId}
                    onChange={(event) =>
                      updateDraft("assetId", event.currentTarget.value)
                    }
                    required
                    disabled={
                      isSaving || readyAssets.length === 0 || editor.mode === "edit"
                    }
                    title={
                      editor.mode === "edit"
                        ? "案例的来源资产在创建后不可更换"
                        : undefined
                    }
                  >
                    <option value="">选择已入库资产</option>
                    {readyAssets.map((asset) => (
                      <option key={asset.id} value={asset.id}>
                        {asset.originalName} · {ASSET_KIND[asset.kind].label} · {formatBytes(asset.sizeBytes)}
                      </option>
                    ))}
                  </select>
                  {readyAssets.length === 0 && (
                    <small className="case-library__field-warning">
                      <AlertCircle size={12} aria-hidden="true" />
                      暂无可用的 ready 资产
                    </small>
                  )}
                </label>

                <label className="case-library__field case-library__field--wide">
                  <span>案例标题 <em>必填</em></span>
                  <input
                    type="text"
                    value={editor.draft.title}
                    onChange={(event) => updateDraft("title", event.currentTarget.value)}
                    maxLength={120}
                    required
                    disabled={isSaving}
                    placeholder="输入可检索的案例标题"
                  />
                </label>

                <label className="case-library__field case-library__field--wide">
                  <span>客户 <em>必填</em></span>
                  <input
                    type="text"
                    value={editor.draft.clientName}
                    onChange={(event) =>
                      updateDraft("clientName", event.currentTarget.value)
                    }
                    maxLength={100}
                    required
                    disabled={isSaving}
                    list="case-library-client-options"
                    placeholder="输入或选择客户"
                  />
                  <datalist id="case-library-client-options">
                    {clients.map((client) => (
                      <option key={client} value={client} />
                    ))}
                  </datalist>
                </label>

                <label className="case-library__field">
                  <span>内容类型</span>
                  <select
                    value={editor.draft.contentType}
                    onChange={(event) =>
                      updateDraft(
                        "contentType",
                        event.currentTarget.value as CaseRecord["contentType"],
                      )
                    }
                    disabled={isSaving}
                  >
                    {CONTENT_TYPES.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="case-library__field">
                  <span>表现形式</span>
                  <select
                    value={editor.draft.presentation}
                    onChange={(event) =>
                      updateDraft(
                        "presentation",
                        event.currentTarget.value as CaseRecord["presentation"],
                      )
                    }
                    disabled={isSaving}
                  >
                    {PRESENTATIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>

                <div className="case-library__field case-library__field--wide">
                  <span>内容属性</span>
                  <div className="case-library__binary-options">
                    <label>
                      <input
                        type="checkbox"
                        checked={editor.draft.hasActors}
                        onChange={(event) =>
                          updateDraft("hasActors", event.currentTarget.checked)
                        }
                        disabled={isSaving}
                      />
                      <span aria-hidden="true" />
                      <UserRound size={14} aria-hidden="true" />
                      有演员
                    </label>
                    <label>
                      <input
                        type="checkbox"
                        checked={editor.draft.isAigc}
                        onChange={(event) =>
                          updateDraft("isAigc", event.currentTarget.checked)
                        }
                        disabled={isSaving}
                      />
                      <span aria-hidden="true" />
                      <Sparkles size={14} aria-hidden="true" />
                      含 AIGC
                    </label>
                  </div>
                </div>

                <div className="case-library__field case-library__field--wide">
                  <span>质量等级</span>
                  <div className="case-library__quality-options">
                    {QUALITY_TIERS.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        className={
                          editor.draft.qualityTier === option.value
                            ? `is-active is-${option.value}`
                            : undefined
                        }
                        aria-pressed={editor.draft.qualityTier === option.value}
                        onClick={() => updateDraft("qualityTier", option.value)}
                        disabled={isSaving}
                      >
                        <Star size={13} aria-hidden="true" />
                        {option.label}
                      </button>
                    ))}
                  </div>
                </div>

                <label className="case-library__field case-library__field--wide">
                  <span>标签</span>
                  <input
                    type="text"
                    value={editor.draft.tags}
                    onChange={(event) => updateDraft("tags", event.currentTarget.value)}
                    maxLength={300}
                    disabled={isSaving}
                    placeholder="空间, 人物, 高级感"
                  />
                </label>

                <label className="case-library__field case-library__field--wide">
                  <span>备注</span>
                  <textarea
                    value={editor.draft.notes}
                    onChange={(event) => updateDraft("notes", event.currentTarget.value)}
                    maxLength={1000}
                    rows={4}
                    disabled={isSaving}
                    placeholder="记录适用场景、亮点或复用注意事项"
                  />
                </label>
              </div>

              <footer className="case-library__editor-footer">
                <button type="button" onClick={onCloseEditor} disabled={isSaving}>
                  取消
                </button>
                <button
                  type="submit"
                  className="case-library__save-button"
                  disabled={!editorCanSave || isSaving}
                >
                  {isSaving ? (
                    <LoaderCircle size={15} className="case-library__spin" />
                  ) : (
                    <Save size={15} />
                  )}
                  {isSaving ? "正在保存" : editor.mode === "create" ? "创建案例" : "保存修改"}
                </button>
              </footer>
            </form>
          </aside>
        )}
      </div>
    </section>
  );
}
