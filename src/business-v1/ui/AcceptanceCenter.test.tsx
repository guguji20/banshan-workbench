import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { BusinessAcceptanceBatchRecord } from "../../generated/bsaigc/BusinessAcceptanceBatchRecord";
import type { BusinessAcceptanceMaterialKind } from "../../generated/bsaigc/BusinessAcceptanceMaterialKind";
import type { BusinessDocumentRecord } from "../../generated/bsaigc/BusinessDocumentRecord";
import type { BusinessWorkspaceRecord } from "../../generated/bsaigc/BusinessWorkspaceRecord";
import {
  AcceptanceCenter,
  acceptanceAssetMatchesRequirement,
  acceptanceDocumentAction,
  acceptanceRequirementProgress,
} from "./AcceptanceCenter";

const REQUIREMENT = {
  id: "requirement-video",
  label: "成片交付",
  kind: "video",
  requiredGroupCount: 2,
} as const;

const ACCEPTANCE_ASSET_CANDIDATES = [
  { id: "asset-script", name: "Asset script", kind: "script" },
  { id: "asset-video", name: "Asset video", kind: "video" },
  { id: "asset-screenshot", name: "Asset screenshot", kind: "screenshot" },
  { id: "asset-behind-the-scenes", name: "Asset behind the scenes", kind: "behindTheScenes" },
  { id: "asset-behind-the-scenes-alias", name: "Asset behind the scenes alias", kind: "behind_the_scenes" },
  { id: "asset-publishing-data", name: "Asset publishing data", kind: "publishingData" },
  { id: "asset-publishing-data-alias", name: "Asset publishing data alias", kind: "publishing-data" },
  { id: "asset-invoice", name: "Asset invoice", kind: "invoice" },
  { id: "asset-proof", name: "Asset proof", kind: "proof" },
  { id: "asset-other", name: "Asset other", kind: "other" },
  { id: "asset-document", name: "Asset document", kind: "document" },
  { id: "asset-image", name: "Asset image", kind: "image" },
] as const;

const ACCEPTANCE_ASSET_FILTER_CASES: ReadonlyArray<{
  requirementKind: BusinessAcceptanceMaterialKind;
  matchingKinds: readonly string[];
}> = [
  { requirementKind: "script", matchingKinds: ["script", "document"] },
  { requirementKind: "video", matchingKinds: ["video"] },
  { requirementKind: "screenshot", matchingKinds: ["screenshot", "image"] },
  { requirementKind: "behindTheScenes", matchingKinds: ["behindTheScenes", "behind_the_scenes", "image", "video"] },
  { requirementKind: "publishingData", matchingKinds: ["publishingData", "publishing-data", "document", "image"] },
  { requirementKind: "invoice", matchingKinds: ["invoice", "document"] },
  { requirementKind: "proof", matchingKinds: ["proof", "document", "image"] },
  { requirementKind: "other", matchingKinds: ["other"] },
];

function documentFixture(status: BusinessDocumentRecord["status"]): BusinessDocumentRecord {
  return {
    id: "acceptance-document-1",
    kind: "videoCompletionAcceptance",
    sequenceNumber: 1,
    documentNumber: "YS-2026-001",
    title: "视频制作完成验收单",
    status,
    snapshot: { acceptanceBatchId: "acceptance-batch-1" },
    outputAssetId: status === "generated" ? "asset-acceptance-result" : null,
    outputFormat: status === "generated" ? "docx" : null,
    updatedAt: 200,
  } as unknown as BusinessDocumentRecord;
}

function batchFixture(isReady: boolean): BusinessAcceptanceBatchRecord {
  return {
    id: "acceptance-batch-1",
    workspaceId: "workspace-1",
    label: "第一阶段验收",
    requirements: [REQUIREMENT],
    outputSpecs: [],
    materials: [{
      id: "material-video-1",
      batchId: "acceptance-batch-1",
      requirementId: REQUIREMENT.id,
      assetId: "asset-video-1",
      kind: "video",
      groupKey: "video-01",
      confirmed: true,
      duplicateOfMaterialId: null,
      notes: "",
      revision: 1,
      createdAt: 100,
      updatedAt: 100,
    }],
    readiness: {
      isReady,
      blockers: isReady ? [] : [{
        code: "missing_material_group",
        requirementId: REQUIREMENT.id,
        requirementLabel: REQUIREMENT.label,
        requiredGroupCount: 2,
        providedGroupCount: 1,
        missingGroupCount: 1,
      }],
    },
    documentIds: ["acceptance-document-1"],
    status: "collecting",
    revision: 3,
    createdAt: 100,
    updatedAt: 200,
  };
}

function workspaceFixture(
  batch: BusinessAcceptanceBatchRecord | null,
  documentStatus: BusinessDocumentRecord["status"] = "draft",
): BusinessWorkspaceRecord {
  return {
    id: "workspace-1",
    projectId: "project-1",
    acceptanceBatches: batch ? [batch] : [],
    documents: batch ? [documentFixture(documentStatus)] : [],
  } as BusinessWorkspaceRecord;
}

function props(workspace: BusinessWorkspaceRecord) {
  return {
    workspace,
    assets: [
      { id: "asset-video-1", name: "成片 01", kind: "video" },
      { id: "asset-video-2", name: "成片 02", kind: "video" },
      { id: "asset-document-1", name: "报价 PDF", kind: "document" },
      { id: "asset-image-1", name: "现场截图", kind: "image" },
    ],
    onCreateBatch: vi.fn(),
    onAddMaterial: vi.fn(),
    onPrepare: vi.fn(),
    onAdvanceDocument: vi.fn(),
    onOpenAsset: vi.fn(),
    onClose: vi.fn(),
  };
}

type TestElement = ReactElement<Record<string, unknown>>;
type AcceptanceCenterTestProps = ReturnType<typeof props>;

function isTestElement(node: ReactNode): node is TestElement {
  return typeof node === "object" && node !== null && "type" in node && "props" in node;
}

function elementText(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(elementText).join("");
  if (!isTestElement(node)) return "";
  return elementText(node.props.children as ReactNode);
}

function findElement(root: ReactNode, predicate: (element: TestElement) => boolean): TestElement {
  let match: TestElement | null = null;
  const visit = (node: ReactNode) => {
    if (match) return;
    if (Array.isArray(node)) {
      node.forEach(visit);
      return;
    }
    if (!isTestElement(node)) return;
    if (predicate(node)) {
      match = node;
      return;
    }
    visit(node.props.children as ReactNode);
  };
  visit(root);
  if (!match) throw new Error("matching element not found");
  return match;
}

function findButton(root: ReactNode, label: string): TestElement {
  return findElement(root, (element) => element.type === "button" && elementText(element).includes(label));
}

function invokeHandler(element: TestElement, name: string, event?: unknown): void {
  const handler = element.props[name];
  if (typeof handler !== "function") throw new Error(`missing ${name} handler`);
  (handler as (value?: unknown) => unknown)(event);
}

function deferredAction() {
  let resolve!: () => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function flushPromises(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

async function createInteractiveHarness(
  workspace: BusinessWorkspaceRecord,
  overrides: Partial<AcceptanceCenterTestProps> = {},
) {
  const states: unknown[] = [];
  const references: Array<{ current: unknown }> = [];
  let stateCursor = 0;
  let referenceCursor = 0;
  const useState = <Value,>(initialValue: Value | (() => Value)) => {
    const stateIndex = stateCursor++;
    if (!(stateIndex in states)) {
      states[stateIndex] = typeof initialValue === "function"
        ? (initialValue as () => Value)()
        : initialValue;
    }
    const setState = (nextValue: Value | ((current: Value) => Value)) => {
      const currentValue = states[stateIndex] as Value;
      states[stateIndex] = typeof nextValue === "function"
        ? (nextValue as (current: Value) => Value)(currentValue)
        : nextValue;
    };
    return [states[stateIndex] as Value, setState] as const;
  };
  const useRef = <Value,>(initialValue: Value) => {
    const referenceIndex = referenceCursor++;
    if (!(referenceIndex in references)) references[referenceIndex] = { current: initialValue };
    return references[referenceIndex] as { current: Value };
  };
  const useEffect = () => undefined;
  const useMemo = <Value,>(factory: () => Value) => factory();

  vi.resetModules();
  vi.doMock("react", async () => {
    const actual = await vi.importActual<typeof import("react")>("react");
    return { ...actual, useState, useRef, useEffect, useMemo };
  });
  const { AcceptanceCenter: InteractiveAcceptanceCenter } = await import("./AcceptanceCenter");
  const componentProps = { ...props(workspace), ...overrides };
  const render = (): ReactNode => {
    stateCursor = 0;
    referenceCursor = 0;
    return InteractiveAcceptanceCenter(componentProps);
  };

  return {
    componentProps,
    render,
    dispose() {
      vi.doUnmock("react");
      vi.resetModules();
    },
  };
}

describe("AcceptanceCenter", () => {
  it("offers batch creation when the workspace has no acceptance batch", () => {
    const html = renderToStaticMarkup(<AcceptanceCenter {...props(workspaceFixture(null))} />);

    expect(html).toContain("还没有验收批次");
    expect(html).toContain("批次名称");
    expect(html).toContain("创建验收批次");
  });

  it("shows one missing group as a red blocker and disables document approval", () => {
    const batch = batchFixture(false);
    const workspace = workspaceFixture(batch, "inReview");
    const html = renderToStaticMarkup(<AcceptanceCenter {...props(workspace)} />);
    const prepareButton = html.match(/<button[^>]*>准备验收草稿<\/button>/)?.[0] ?? "";

    expect(acceptanceRequirementProgress(batch, batch.requirements[0])).toEqual({
      required: 2,
      provided: 1,
      missing: 1,
    });
    expect(html).toContain("bw-acceptance-blocker");
    expect(html).toContain("缺少 1 组确认素材");
    expect(html).toMatch(/disabled=""[^>]*>批准验收<\/button>/);
    expect(prepareButton).not.toContain("disabled");
  });

  it("allows the next document step once readiness is true", () => {
    const batch = batchFixture(true);
    batch.materials.push({
      ...batch.materials[0],
      id: "material-video-2",
      assetId: "asset-video-2",
      groupKey: "video-02",
    });
    const document = documentFixture("draft");
    const html = renderToStaticMarkup(<AcceptanceCenter {...props(workspaceFixture(batch, "draft"))} />);

    expect(acceptanceRequirementProgress(batch, batch.requirements[0])).toEqual({
      required: 2,
      provided: 2,
      missing: 0,
    });
    expect(acceptanceDocumentAction(document, true)).toMatchObject({
      kind: "advance",
      label: "提交复核",
      disabled: false,
    });
    expect(html).toMatch(/<button[^>]*>提交复核<\/button>/);
    expect(html).not.toMatch(/disabled=""[^>]*>提交复核<\/button>/);
  });

  it("exposes the generated result asset through the open action", () => {
    const document = documentFixture("generated");
    const action = acceptanceDocumentAction(document, true);
    const html = renderToStaticMarkup(
      <AcceptanceCenter {...props(workspaceFixture(batchFixture(true), "generated"))} />,
    );

    expect(action).toEqual({
      kind: "open",
      label: "打开成果",
      disabled: false,
      assetId: "asset-acceptance-result",
    });
    expect(html).toContain("DOCX");
    expect(html).toMatch(/<button[^>]*>打开成果<\/button>/);
  });

  it.each(ACCEPTANCE_ASSET_FILTER_CASES)(
    "filters $requirementKind assets by the complete compatibility matrix",
    ({ requirementKind, matchingKinds }) => {
      const batch = batchFixture(false);
      batch.requirements = [{
        id: `requirement-${requirementKind}`,
        label: `Requirement ${requirementKind}`,
        kind: requirementKind,
        requiredGroupCount: 1,
      }];
      batch.materials = [];
      batch.readiness = {
        isReady: false,
        blockers: [{
          code: "missing_material_group",
          requirementId: `requirement-${requirementKind}`,
          requirementLabel: `Requirement ${requirementKind}`,
          requiredGroupCount: 1,
          providedGroupCount: 0,
          missingGroupCount: 1,
        }],
      };
      const html = renderToStaticMarkup(
        <AcceptanceCenter
          {...props(workspaceFixture(batch))}
          assets={ACCEPTANCE_ASSET_CANDIDATES}
        />,
      );

      for (const asset of ACCEPTANCE_ASSET_CANDIDATES) {
        const shouldMatch = matchingKinds.includes(asset.kind);

        expect(acceptanceAssetMatchesRequirement(asset, requirementKind)).toBe(shouldMatch);
        if (shouldMatch) {
          expect(html).toContain(`${asset.name} · ${asset.kind}`);
        } else {
          expect(html).not.toContain(`${asset.name} · ${asset.kind}`);
        }
      }
    },
  );

  it("locks rapid batch creation and preserves the label until a successful retry", async () => {
    const firstAttempt = deferredAction();
    const onCreateBatch = vi.fn()
      .mockImplementationOnce(() => firstAttempt.promise)
      .mockResolvedValueOnce(undefined);
    const harness = await createInteractiveHarness(workspaceFixture(null), { onCreateBatch });

    try {
      let root = harness.render();
      const input = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：第一阶段交付验收");
      invokeHandler(input, "onChange", { target: { value: " 第一阶段验收 " } });

      root = harness.render();
      const createButton = findButton(root, "创建验收批次");
      invokeHandler(createButton, "onClick");
      invokeHandler(createButton, "onClick");

      expect(onCreateBatch).toHaveBeenCalledTimes(1);
      expect(onCreateBatch).toHaveBeenCalledWith("第一阶段验收");

      root = harness.render();
      const pendingButton = findButton(root, "创建中…");
      const pendingInput = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：第一阶段交付验收");
      expect(pendingButton.props.disabled).toBe(true);
      expect(pendingButton.props["aria-busy"]).toBe(true);
      expect(pendingInput.props.value).toBe(" 第一阶段验收 ");
      expect(pendingInput.props.disabled).toBe(true);

      firstAttempt.reject(new Error("网络中断"));
      await flushPromises();

      root = harness.render();
      const retryButton = findButton(root, "创建验收批次");
      const retainedInput = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：第一阶段交付验收");
      expect(retryButton.props.disabled).toBe(false);
      expect(retainedInput.props.value).toBe(" 第一阶段验收 ");
      expect(elementText(root)).toContain("创建验收批次失败：网络中断");

      invokeHandler(retryButton, "onClick");
      await flushPromises();

      root = harness.render();
      const clearedInput = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：第一阶段交付验收");
      expect(onCreateBatch).toHaveBeenCalledTimes(2);
      expect(clearedInput.props.value).toBe("");
    } finally {
      harness.dispose();
    }
  });

  it("locks rapid material binding and only clears the draft after success", async () => {
    const firstAttempt = deferredAction();
    const onAddMaterial = vi.fn()
      .mockImplementationOnce(() => firstAttempt.promise)
      .mockResolvedValueOnce(undefined);
    const harness = await createInteractiveHarness(workspaceFixture(batchFixture(false)), { onAddMaterial });

    try {
      let root = harness.render();
      const assetSelect = findElement(root, (element) => element.type === "select" && element.props.value === "");
      const groupInput = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：video-01");
      const confirmation = findElement(root, (element) => element.type === "input" && element.props.type === "checkbox");
      invokeHandler(assetSelect, "onChange", { target: { value: "asset-video-2" } });
      invokeHandler(groupInput, "onChange", { target: { value: " video-02 " } });
      invokeHandler(confirmation, "onChange", { target: { checked: true } });

      root = harness.render();
      const bindButton = findButton(root, "绑定素材");
      invokeHandler(bindButton, "onClick");
      invokeHandler(bindButton, "onClick");

      expect(onAddMaterial).toHaveBeenCalledTimes(1);
      expect(onAddMaterial).toHaveBeenCalledWith("acceptance-batch-1", expect.objectContaining({
        requirementId: REQUIREMENT.id,
        assetId: "asset-video-2",
        kind: "video",
        groupKey: "video-02",
        confirmed: true,
      }));

      root = harness.render();
      const pendingButton = findButton(root, "绑定中…");
      const pendingSelect = findElement(root, (element) => element.type === "select" && element.props.value === "asset-video-2");
      const pendingGroup = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：video-01");
      expect(pendingButton.props.disabled).toBe(true);
      expect(pendingSelect.props.disabled).toBe(true);
      expect(pendingGroup.props.value).toBe(" video-02 ");
      expect(pendingGroup.props.disabled).toBe(true);

      firstAttempt.reject(new Error("资产写入失败"));
      await flushPromises();

      root = harness.render();
      const retryButton = findButton(root, "绑定素材");
      const retainedSelect = findElement(root, (element) => element.type === "select" && element.props.value === "asset-video-2");
      const retainedGroup = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：video-01");
      const retainedConfirmation = findElement(root, (element) => element.type === "input" && element.props.type === "checkbox");
      expect(retryButton.props.disabled).toBe(false);
      expect(retainedSelect.props.value).toBe("asset-video-2");
      expect(retainedGroup.props.value).toBe(" video-02 ");
      expect(retainedConfirmation.props.checked).toBe(true);
      expect(elementText(root)).toContain("绑定素材失败：资产写入失败");

      invokeHandler(retryButton, "onClick");
      await flushPromises();

      root = harness.render();
      const clearedSelect = findElement(root, (element) => element.type === "select" && element.props.value === "");
      const clearedGroup = findElement(root, (element) => element.type === "input" && element.props.placeholder === "例如：video-01");
      const clearedConfirmation = findElement(root, (element) => element.type === "input" && element.props.type === "checkbox");
      expect(onAddMaterial).toHaveBeenCalledTimes(2);
      expect(clearedSelect.props.value).toBe("");
      expect(clearedGroup.props.value).toBe("");
      expect(clearedConfirmation.props.checked).toBe(false);
    } finally {
      harness.dispose();
    }
  });
  it("locks rapid draft preparation and exposes a retry after failure", async () => {
    const firstAttempt = deferredAction();
    const onPrepare = vi.fn()
      .mockImplementationOnce(() => firstAttempt.promise)
      .mockResolvedValueOnce(undefined);
    const harness = await createInteractiveHarness(workspaceFixture(batchFixture(false)), { onPrepare });

    try {
      let root = harness.render();
      const prepareButton = findButton(root, "准备验收草稿");
      invokeHandler(prepareButton, "onClick");
      invokeHandler(prepareButton, "onClick");

      expect(onPrepare).toHaveBeenCalledTimes(1);
      expect(onPrepare).toHaveBeenCalledWith("acceptance-batch-1");

      root = harness.render();
      const pendingButton = findButton(root, "准备中…");
      expect(pendingButton.props.disabled).toBe(true);
      expect(pendingButton.props["aria-busy"]).toBe(true);

      firstAttempt.reject(new Error("草稿服务不可用"));
      await flushPromises();

      root = harness.render();
      const retryButton = findButton(root, "准备验收草稿");
      expect(retryButton.props.disabled).toBe(false);
      expect(elementText(root)).toContain("准备验收草稿失败：草稿服务不可用");

      invokeHandler(retryButton, "onClick");
      await flushPromises();
      expect(onPrepare).toHaveBeenCalledTimes(2);
    } finally {
      harness.dispose();
    }
  });

  it("shows structured host errors without discarding their message", async () => {
    const onPrepare = vi.fn().mockRejectedValue({
      code: "BUSINESS_ACCEPTANCE_ASSET_KIND_MISMATCH",
      message: "验收素材类型与要求不匹配",
      retryable: false,
    });
    const harness = await createInteractiveHarness(workspaceFixture(batchFixture(false)), { onPrepare });

    try {
      invokeHandler(findButton(harness.render(), "准备验收草稿"), "onClick");
      await flushPromises();

      expect(elementText(harness.render())).toContain("准备验收草稿失败：验收素材类型与要求不匹配");
    } finally {
      harness.dispose();
    }
  });

  it("locks rapid document advancement and allows retry", async () => {
    const firstAttempt = deferredAction();
    const onAdvanceDocument = vi.fn()
      .mockImplementationOnce(() => firstAttempt.promise)
      .mockResolvedValueOnce(undefined);
    const harness = await createInteractiveHarness(
      workspaceFixture(batchFixture(true), "draft"),
      { onAdvanceDocument },
    );

    try {
      let root = harness.render();
      const advanceButton = findButton(root, "提交复核");
      invokeHandler(advanceButton, "onClick");
      invokeHandler(advanceButton, "onClick");

      expect(onAdvanceDocument).toHaveBeenCalledTimes(1);
      expect(onAdvanceDocument).toHaveBeenCalledWith("acceptance-document-1");

      root = harness.render();
      const pendingButton = findButton(root, "提交复核中…");
      expect(pendingButton.props.disabled).toBe(true);

      firstAttempt.reject(new Error("文档版本冲突"));
      await flushPromises();

      root = harness.render();
      const retryButton = findButton(root, "提交复核");
      expect(retryButton.props.disabled).toBe(false);
      expect(elementText(root)).toContain("提交复核失败：文档版本冲突");

      invokeHandler(retryButton, "onClick");
      await flushPromises();
      expect(onAdvanceDocument).toHaveBeenCalledTimes(2);
    } finally {
      harness.dispose();
    }
  });

  it("locks rapid result opening and allows retry", async () => {
    const firstAttempt = deferredAction();
    const onOpenAsset = vi.fn()
      .mockImplementationOnce(() => firstAttempt.promise)
      .mockResolvedValueOnce(undefined);
    const harness = await createInteractiveHarness(
      workspaceFixture(batchFixture(true), "generated"),
      { onOpenAsset },
    );

    try {
      let root = harness.render();
      const openButton = findButton(root, "打开成果");
      invokeHandler(openButton, "onClick");
      invokeHandler(openButton, "onClick");

      expect(onOpenAsset).toHaveBeenCalledTimes(1);
      expect(onOpenAsset).toHaveBeenCalledWith("asset-acceptance-result");

      root = harness.render();
      const pendingButton = findButton(root, "打开成果中…");
      expect(pendingButton.props.disabled).toBe(true);

      firstAttempt.reject(new Error("文件暂不可用"));
      await flushPromises();

      root = harness.render();
      const retryButton = findButton(root, "打开成果");
      expect(retryButton.props.disabled).toBe(false);
      expect(elementText(root)).toContain("打开成果失败：文件暂不可用");

      invokeHandler(retryButton, "onClick");
      await flushPromises();
      expect(onOpenAsset).toHaveBeenCalledTimes(2);
    } finally {
      harness.dispose();
    }
  });});
