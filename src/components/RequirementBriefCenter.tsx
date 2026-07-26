import {
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
} from "react";
import {
  AlertCircle,
  CalendarClock,
  Check,
  CheckCircle2,
  Circle,
  ClipboardList,
  FilePlus2,
  FolderSearch2,
  LoaderCircle,
  MessageCircleQuestion,
  MinusCircle,
  RefreshCw,
  RotateCcw,
  Save,
  Send,
  ShieldCheck,
} from "lucide-react";
import type { CaseRecord } from "../generated/bsaigc/CaseRecord";
import type { ProjectRecord } from "../generated/bsaigc/ProjectRecord";
import type { RequirementAnswerDisposition } from "../generated/bsaigc/RequirementAnswerDisposition";
import type { RequirementBriefContent } from "../generated/bsaigc/RequirementBriefContent";
import type { RequirementBriefRecord } from "../generated/bsaigc/RequirementBriefRecord";
import type { RequirementBriefStatus } from "../generated/bsaigc/RequirementBriefStatus";
import type { RequirementQuestionAnswer } from "../generated/bsaigc/RequirementQuestionAnswer";
import {
  hasFollowUp,
  reviewMissing,
  sameRequirementBriefDraft,
  type RequirementBriefDraft,
} from "../requirementBriefDrafts";
import "./RequirementBriefCenter.css";

export interface RequirementBriefCenterProps {
  projects: readonly ProjectRecord[];
  briefs: readonly RequirementBriefRecord[];
  cases: readonly CaseRecord[];
  selectedProjectId: string | null;
  draft: RequirementBriefDraft | null;
  hasConflict?: boolean;
  isRefreshing?: boolean;
  isSaving?: boolean;
  error?: string | null;
  onSelectProject: (projectId: string) => void;
  onDraftChange: (draft: RequirementBriefDraft) => void;
  onCreate: (projectId: string) => void;
  onSave: (record: RequirementBriefRecord, draft: RequirementBriefDraft) => void;
  onChangeStatus: (
    record: RequirementBriefRecord,
    status: RequirementBriefStatus,
    draft: RequirementBriefDraft,
  ) => void;
  onRefresh: () => void;
  onReloadConflict: (record: RequirementBriefRecord) => void;
  onRebaseConflict: (record: RequirementBriefRecord) => void;
}

const STATUS_LABELS: Record<RequirementBriefStatus, string> = {
  interviewing: "访谈中",
  review: "待确认",
  confirmed: "已确认",
};

const DISPOSITIONS: ReadonlyArray<{
  value: RequirementAnswerDisposition;
  label: string;
  icon: typeof Circle;
}> = [
  { value: "unanswered", label: "未处理", icon: Circle },
  { value: "answered", label: "已回答", icon: CheckCircle2 },
  { value: "followUp", label: "需追问", icon: MessageCircleQuestion },
  { value: "notApplicable", label: "不适用", icon: MinusCircle },
];

function lines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function toDateTimeLocal(value: number | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

function fromDateTimeLocal(value: string): number | null {
  if (!value) return null;
  const timestamp = new Date(value).getTime();
  return Number.isFinite(timestamp) ? timestamp : null;
}

function formatUpdatedAt(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function ListField({
  label,
  value,
  placeholder,
  disabled,
  important = false,
  onChange,
}: {
  label: string;
  value: readonly string[];
  placeholder: string;
  disabled: boolean;
  important?: boolean;
  onChange: (value: string[]) => void;
}) {
  const [rawValue, setRawValue] = useState(() => value.join("\n"));

  useEffect(() => {
    setRawValue((current) => {
      const currentLines = lines(current);
      const matchesValue =
        currentLines.length === value.length &&
        currentLines.every((item, index) => item === value[index]);

      return matchesValue ? current : value.join("\n");
    });
  }, [value]);

  return (
    <label
      className={
        important
          ? "requirement-brief__field is-important"
          : "requirement-brief__field"
      }
    >
      <span>{label}</span>
      <textarea
        rows={important ? 4 : 3}
        value={rawValue}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(event) => {
          const nextRawValue = event.currentTarget.value;
          setRawValue(nextRawValue);
          onChange(lines(nextRawValue));
        }}
      />
      <small>{value.length} 项</small>
    </label>
  );
}

export function RequirementBriefCenter({
  projects,
  briefs,
  cases,
  selectedProjectId,
  draft,
  hasConflict = false,
  isRefreshing = false,
  isSaving = false,
  error = null,
  onSelectProject,
  onDraftChange,
  onCreate,
  onSave,
  onChangeStatus,
  onRefresh,
  onReloadConflict,
  onRebaseConflict,
}: RequirementBriefCenterProps) {
  const [activeQuestionId, setActiveQuestionId] = useState<string | null>(null);
  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? null;
  const record =
    briefs.find((brief) => brief.projectId === selectedProjectId) ?? null;
  const projectCases = useMemo(
    () =>
      cases.filter(
        (item) => item.projectId === null || item.projectId === selectedProjectId,
      ),
    [cases, selectedProjectId],
  );
  const missing = draft ? reviewMissing(draft) : [];
  const followUp = draft ? hasFollowUp(draft) : false;
  const readOnly = record?.status === "confirmed";
  const editorDisabled = readOnly || isSaving || hasConflict;
  const isDirty =
    record !== null && draft !== null
      ? !sameRequirementBriefDraft(
          { answers: record.answers, content: record.content },
          draft,
        )
      : false;
  const transitionBlockReason = hasConflict
    ? "请先处理版本冲突"
    : isDirty
      ? "请先保存更改"
      : missing.length > 0
        ? `还有 ${missing.length} 项必填内容待补齐（${missing.slice(0, 3).join("、")}${missing.length > 3 ? " 等" : ""}）`
        : null;
  const answeredCount =
    draft?.answers.filter((answer) => answer.disposition !== "unanswered")
      .length ?? 0;
  const activeQuestion =
    draft?.answers.find((answer) => answer.questionId === activeQuestionId) ??
    draft?.answers[0] ??
    null;

  useEffect(() => {
    if (!draft || draft.answers.length === 0) {
      setActiveQuestionId(null);
      return;
    }
    setActiveQuestionId((current) => {
      if (draft.answers.some((answer) => answer.questionId === current)) {
        return current;
      }
      return (
        draft.answers.find(
          (answer) => answer.disposition === "unanswered",
        )?.questionId ?? draft.answers[0]?.questionId ?? null
      );
    });
  }, [record?.id, record?.revision]);

  function updateContent<Key extends keyof RequirementBriefContent>(
    key: Key,
    value: RequirementBriefContent[Key],
  ) {
    if (!draft || editorDisabled) return;
    onDraftChange({
      ...draft,
      content: { ...draft.content, [key]: value },
    });
  }

  function updateQuestion(
    questionId: string,
    patch: Partial<
      Pick<RequirementQuestionAnswer, "answer" | "disposition">
    >,
  ) {
    if (!draft || editorDisabled) return;
    onDraftChange({
      ...draft,
      answers: draft.answers.map((answer) =>
        answer.questionId === questionId ? { ...answer, ...patch } : answer,
      ),
    });
  }

  function toggleReference(caseId: string) {
    if (!draft || editorDisabled) return;
    const selected = draft.content.referenceCaseIds.includes(caseId);
    updateContent(
      "referenceCaseIds",
      selected
        ? draft.content.referenceCaseIds.filter((id) => id !== caseId)
        : [...draft.content.referenceCaseIds, caseId],
    );
  }

  return (
    <section className="requirement-brief" aria-labelledby="requirement-title">
      <header className="requirement-brief__header">
        <h1 id="requirement-title">需求访谈</h1>
        <button
          type="button"
          className="requirement-brief__icon-button"
          onClick={onRefresh}
          disabled={isRefreshing}
          title="刷新需求 Brief"
          aria-label="刷新需求 Brief"
        >
          <RefreshCw
            size={16}
            className={isRefreshing ? "is-spinning" : undefined}
          />
        </button>
      </header>

      {error && (
        <div className="requirement-brief__error" role="alert">
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      <div className="requirement-brief__layout">
        <aside className="requirement-brief__projects" aria-label="需求项目">
          <div className="requirement-brief__rail-heading">
            <strong>项目</strong>
            <span>{projects.length}</span>
          </div>
          <div className="requirement-brief__project-list">
            {projects.map((project) => {
              const brief = briefs.find((item) => item.projectId === project.id);
              return (
                <button
                  type="button"
                  key={project.id}
                  className={
                    project.id === selectedProjectId ? "is-selected" : undefined
                  }
                  onClick={() => onSelectProject(project.id)}
                >
                  <span>
                    <strong>{project.name}</strong>
                    <small>{project.clientName}</small>
                  </span>
                  <em className={brief ? `is-${brief.status}` : undefined}>
                    {brief ? STATUS_LABELS[brief.status] : "未开始"}
                  </em>
                </button>
              );
            })}
          </div>
        </aside>

        <div className="requirement-brief__workspace">
          {!selectedProject ? (
            <div className="requirement-brief__empty">
              <ClipboardList size={30} strokeWidth={1.5} />
              <span>选择左侧项目开始整理需求</span>
            </div>
          ) : !record || !draft ? (
            <div className="requirement-brief__empty">
              {isSaving ? (
                <LoaderCircle size={30} className="is-spinning" />
              ) : (
                <FilePlus2 size={30} strokeWidth={1.5} />
              )}
              <button
                type="button"
                className="requirement-brief__primary-button"
                disabled={isSaving}
                onClick={() => onCreate(selectedProject.id)}
              >
                {isSaving
                  ? "正在建立需求访谈"
                  : `为 ${selectedProject.name} 建立需求访谈`}
              </button>
            </div>
          ) : (
            <>
              <div className="requirement-brief__toolbar">
                <div>
                  <span>{selectedProject.clientName}</span>
                  <h2>{selectedProject.name}</h2>
                </div>
                <div className="requirement-brief__actions">
                  <span
                    className={`requirement-brief__status is-${record.status}`}
                  >
                    {record.status === "confirmed" ? (
                      <ShieldCheck size={14} />
                    ) : record.status === "review" ? (
                      <Send size={14} />
                    ) : (
                      <MessageCircleQuestion size={14} />
                    )}
                    {STATUS_LABELS[record.status]}
                  </span>
                  {record.status !== "confirmed" && (
                    <button
                      type="button"
                      className="requirement-brief__primary-button"
                      disabled={isSaving || hasConflict}
                      title={hasConflict ? "请先处理版本冲突" : undefined}
                      onClick={() => onSave(record, draft)}
                    >
                      {isSaving ? (
                        <LoaderCircle size={15} className="is-spinning" />
                      ) : (
                        <Save size={15} />
                      )}
                      保存
                    </button>
                  )}
                  {record.status === "interviewing" && (
                    <button
                      type="button"
                      className="requirement-brief__secondary-button"
                      disabled={
                        isSaving || hasConflict || isDirty || missing.length > 0
                      }
                      title={
                        transitionBlockReason
                          ? `${transitionBlockReason}，再提交复核`
                          : undefined
                      }
                      aria-label={
                        transitionBlockReason
                          ? `提交复核（${transitionBlockReason}）`
                          : "提交复核"
                      }
                      onClick={() => onChangeStatus(record, "review", draft)}
                    >
                      <Send size={15} />
                      提交复核
                    </button>
                  )}
                  {record.status === "review" && (
                    <>
                      <button
                        type="button"
                        className="requirement-brief__secondary-button"
                        disabled={isSaving || hasConflict || isDirty}
                        title={
                          transitionBlockReason
                            ? `${transitionBlockReason}，再退回补访`
                            : undefined
                        }
                        aria-label={
                          transitionBlockReason
                            ? `退回补访（${transitionBlockReason}）`
                            : "退回补访"
                        }
                        onClick={() =>
                          onChangeStatus(record, "interviewing", draft)
                        }
                      >
                        <RotateCcw size={15} />
                        退回补访
                      </button>
                      <button
                        type="button"
                        className="requirement-brief__confirm-button"
                        disabled={
                          isSaving ||
                          hasConflict ||
                          isDirty ||
                          missing.length > 0 ||
                          followUp
                        }
                        title={
                          transitionBlockReason
                            ? `${transitionBlockReason}，再确认需求`
                            : undefined
                        }
                        aria-label={
                          transitionBlockReason
                            ? `确认需求（${transitionBlockReason}）`
                            : "确认需求"
                        }
                        onClick={() => onChangeStatus(record, "confirmed", draft)}
                      >
                        <ShieldCheck size={15} />
                        确认需求
                      </button>
                    </>
                  )}
                  {record.status === "confirmed" && (
                    <button
                      type="button"
                      className="requirement-brief__secondary-button"
                      disabled={isSaving}
                      onClick={() => onChangeStatus(record, "review", draft)}
                    >
                      <RotateCcw size={15} />
                      重新打开
                    </button>
                  )}
                </div>
              </div>

              {hasConflict && (
                <div className="requirement-brief__conflict" role="status">
                  <AlertCircle size={17} />
                  <span>
                    <strong>需求 Brief 已有更新版本</strong>
                    <small>选择保留本地内容，或载入最新版本。</small>
                  </span>
                  <button
                    type="button"
                    onClick={() => onRebaseConflict(record)}
                  >
                    保留本地
                  </button>
                  <button
                    type="button"
                    onClick={() => onReloadConflict(record)}
                  >
                    载入最新
                  </button>
                </div>
              )}

              <div className="requirement-brief__gate">
                <div>
                  <CheckCircle2 size={16} />
                  <strong>需求完整度</strong>
                  <span>{missing.length === 0 ? "可提交" : `${missing.length} 项待补`}</span>
                </div>
                <div>
                  <MessageCircleQuestion size={16} />
                  <strong>访谈问题</strong>
                  <span>{answeredCount}/{draft.answers.length}</span>
                </div>
                <div className={followUp ? "has-follow-up" : undefined}>
                  <AlertCircle size={16} />
                  <strong>待追问</strong>
                  <span>
                    {draft.answers.filter((answer) => answer.disposition === "followUp").length}
                  </span>
                </div>
              </div>

              <div className="requirement-brief__body">
                <section className="requirement-brief__interview" aria-label="访谈问题">
                  <div className="requirement-brief__section-title">
                    <div>
                      <MessageCircleQuestion size={17} />
                      <strong>问题清单</strong>
                    </div>
                    <span>{record.questionSetVersion}</span>
                  </div>
                  <div className="requirement-brief__question-tabs">
                    {draft.answers.map((answer, index) => (
                      <button
                        type="button"
                        key={answer.questionId}
                        className={
                          answer.questionId === activeQuestion?.questionId
                            ? `is-active is-${answer.disposition}`
                            : `is-${answer.disposition}`
                        }
                        onClick={() => setActiveQuestionId(answer.questionId)}
                        title={answer.prompt}
                        aria-label={`${index + 1}. ${answer.prompt}${answer.required ? "（必答）" : ""}`}
                      >
                        <span>{index + 1}</span>
                        {answer.required && <em>必</em>}
                      </button>
                    ))}
                  </div>
                  {activeQuestion && (
                    <div className="requirement-brief__question-editor">
                      <div className="requirement-brief__question-prompt">
                        <span>{activeQuestion.required ? "必问" : "补充"}</span>
                        <h3>{activeQuestion.prompt}</h3>
                      </div>
                      <textarea
                        rows={7}
                        value={activeQuestion.answer}
                        disabled={editorDisabled}
                        onChange={(event) => {
                          const answer = event.currentTarget.value;
                          updateQuestion(activeQuestion.questionId, {
                            answer,
                            disposition:
                              answer.trim() &&
                              activeQuestion.disposition === "unanswered"
                                ? "answered"
                                : activeQuestion.disposition,
                          });
                        }}
                      />
                      <div className="requirement-brief__dispositions">
                        {DISPOSITIONS.map((item) => {
                          const Icon = item.icon;
                          return (
                            <button
                              type="button"
                              key={item.value}
                              className={
                                activeQuestion.disposition === item.value
                                  ? "is-selected"
                                  : undefined
                              }
                              disabled={editorDisabled}
                              onClick={() =>
                                // 切回「未处理」时保留已写答案，避免误触丢内容；
                                // 答案文本仍在，重新标记后无需重写。
                                updateQuestion(activeQuestion.questionId, {
                                  disposition: item.value,
                                })
                              }
                            >
                              <Icon size={14} />
                              {item.label}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </section>

                <section className="requirement-brief__summary" aria-label="结构化需求摘要">
                  <div className="requirement-brief__section-title">
                    <div>
                      <ClipboardList size={17} />
                      <strong>Brief 摘要</strong>
                    </div>
                    <span>{readOnly ? "已锁定" : hasConflict ? "待处理冲突" : "编辑中"}</span>
                  </div>
                  <div className="requirement-brief__form">
                    <label className="requirement-brief__field is-important">
                      <span>项目目标</span>
                      <textarea
                        rows={4}
                        value={draft.content.objective}
                        disabled={editorDisabled}
                        onChange={(event) =>
                          updateContent("objective", event.currentTarget.value)
                        }
                      />
                    </label>
                    <label className="requirement-brief__field is-important">
                      <span>目标受众</span>
                      <textarea
                        rows={4}
                        value={draft.content.audience}
                        disabled={editorDisabled}
                        onChange={(event) =>
                          updateContent("audience", event.currentTarget.value)
                        }
                      />
                    </label>
                    <label className="requirement-brief__field is-wide is-important">
                      <span>核心信息</span>
                      <textarea
                        rows={3}
                        value={draft.content.keyMessage}
                        disabled={editorDisabled}
                        onChange={(event) =>
                          updateContent("keyMessage", event.currentTarget.value)
                        }
                      />
                    </label>
                    <ListField
                      label="交付物"
                      value={draft.content.deliverables}
                      placeholder={"90 秒主片\n15 秒切条"}
                      disabled={editorDisabled}
                      important
                      onChange={(value) => updateContent("deliverables", value)}
                    />
                    <ListField
                      label="发布渠道"
                      value={draft.content.channels}
                      placeholder={"视频号\n抖音\n线下大屏"}
                      disabled={editorDisabled}
                      important
                      onChange={(value) => updateContent("channels", value)}
                    />
                    <ListField
                      label="风格关键词"
                      value={draft.content.styleKeywords}
                      placeholder={"克制\n真实质感\n明快节奏"}
                      disabled={editorDisabled}
                      onChange={(value) => updateContent("styleKeywords", value)}
                    />
                    <ListField
                      label="必须出现"
                      value={draft.content.mandatoryItems}
                      placeholder={"品牌 Logo\n核心人物\n指定产品"}
                      disabled={editorDisabled}
                      onChange={(value) => updateContent("mandatoryItems", value)}
                    />
                    <ListField
                      label="约束条件"
                      value={draft.content.constraints}
                      placeholder={"拍摄档期\n场地限制\n合规边界"}
                      disabled={editorDisabled}
                      onChange={(value) => updateContent("constraints", value)}
                    />
                    <ListField
                      label="验收标准"
                      value={draft.content.acceptanceCriteria}
                      placeholder={"验收人\n通过标准\n交付格式"}
                      disabled={editorDisabled}
                      important
                      onChange={(value) =>
                        updateContent("acceptanceCriteria", value)
                      }
                    />
                    <ListField
                      label="风险"
                      value={draft.content.risks}
                      placeholder={"方向未确认\n素材未授权\n档期冲突"}
                      disabled={editorDisabled}
                      onChange={(value) => updateContent("risks", value)}
                    />
                    <label className="requirement-brief__field">
                      <span>交付时间</span>
                      <div className="requirement-brief__date-input">
                        <CalendarClock size={16} />
                        <input
                          type="datetime-local"
                          value={toDateTimeLocal(draft.content.deadlineAt)}
                          disabled={editorDisabled}
                          onChange={(event: ChangeEvent<HTMLInputElement>) =>
                            updateContent(
                              "deadlineAt",
                              fromDateTimeLocal(event.currentTarget.value),
                            )
                          }
                        />
                      </div>
                    </label>
                    <label className="requirement-brief__field">
                      <span>预算备注</span>
                      <textarea
                        rows={3}
                        value={draft.content.budgetNotes}
                        disabled={editorDisabled}
                        onChange={(event) =>
                          updateContent("budgetNotes", event.currentTarget.value)
                        }
                      />
                    </label>
                    <label className="requirement-brief__field is-wide">
                      <span>参考备注</span>
                      <textarea
                        rows={3}
                        value={draft.content.referenceNotes}
                        disabled={editorDisabled}
                        onChange={(event) =>
                          updateContent("referenceNotes", event.currentTarget.value)
                        }
                      />
                    </label>
                  </div>

                  <div className="requirement-brief__references">
                    <div className="requirement-brief__section-title">
                      <div>
                        <FolderSearch2 size={17} />
                        <strong>参考案例</strong>
                      </div>
                      <span>{draft.content.referenceCaseIds.length} 个</span>
                    </div>
                    {projectCases.length === 0 ? (
                      <div className="requirement-brief__reference-empty">
                        暂无可引用案例
                      </div>
                    ) : (
                      <div className="requirement-brief__case-list">
                        {projectCases.map((item) => {
                          const selected = draft.content.referenceCaseIds.includes(
                            item.id,
                          );
                          return (
                            <label key={item.id} className={selected ? "is-selected" : undefined}>
                              <input
                                type="checkbox"
                                checked={selected}
                                disabled={editorDisabled}
                                onChange={() => toggleReference(item.id)}
                              />
                              <span>
                                <strong>{item.title}</strong>
                                <small>{item.clientName}</small>
                              </span>
                              {selected && <Check size={14} />}
                            </label>
                          );
                        })}
                      </div>
                    )}
                  </div>
                </section>
              </div>

              <footer className="requirement-brief__footer">
                <span>最近更新 {formatUpdatedAt(record.updatedAt)}</span>
                {record.confirmedBy && <span>确认人 {record.confirmedBy}</span>}
                <span>{record.id.slice(0, 8)}</span>
              </footer>
            </>
          )}
        </div>
      </div>
    </section>
  );
}
