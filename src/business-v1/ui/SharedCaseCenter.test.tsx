import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { BsaigcClient } from "../../client-sdk";
import type { AppUserRecord } from "../../generated/bsaigc/AppUserRecord";
import type { CaseRecord } from "../../generated/bsaigc/CaseRecord";
import type { SharedCaseDomainEvent } from "../../generated/bsaigc/SharedCaseDomainEvent";
import type { SharedCasePublicationRecord } from "../../generated/bsaigc/SharedCasePublicationRecord";
import * as SharedCaseCenterModule from "./SharedCaseCenter";
import {
  ensureSharedCaseManagerGrant,
  parseSharedCaseGrants,
  SharedCasePanel,
  type SharedCasePanelProps,
} from "./SharedCaseCenter";

const CASE_RECORD: CaseRecord = {
  id: "case-1",
  assetId: "asset-1",
  projectId: "project-1",
  title: "白鹅潭品牌片",
  clientName: "客户甲",
  contentType: "brand",
  presentation: "mixedMedia",
  hasActors: true,
  isAigc: false,
  qualityTier: "premium",
  tags: ["品牌"],
  notes: "内部授权案例",
  revision: 1,
  createdAt: 100,
  updatedAt: 100,
};

const PUBLICATION: SharedCasePublicationRecord = {
  id: "publication-1",
  caseId: CASE_RECORD.id,
  assetId: CASE_RECORD.assetId,
  projectId: CASE_RECORD.projectId,
  title: CASE_RECORD.title,
  clientName: CASE_RECORD.clientName,
  contentSha256: "a".repeat(64),
  remoteObjectKey: "shared/case-1.mp4",
  remoteEtag: "etag-1",
  status: "published",
  publisherUsername: "admin",
  grants: [
    { username: "admin", permissions: ["discover", "preview", "reference", "download"] },
    { username: "alice", permissions: ["discover", "preview", "reference"] },
  ],
  revision: 2,
  createdAt: 100,
  updatedAt: 200,
  publishedAt: 200,
  withdrawnAt: null,
};

const SHARED_CASE_EVENT: SharedCaseDomainEvent = {
  sequence: 41,
  eventId: "shared-case-event-41",
  eventType: "sharedCase.published",
  aggregateId: PUBLICATION.id,
  revision: PUBLICATION.revision,
  occurredAt: 200,
  traceId: "trace-shared-case-41",
  publication: PUBLICATION,
};

const ADMIN_USER: AppUserRecord = {
  username: "admin",
  role: "admin",
  status: "active",
  updatedAt: 200,
};

const MEMBER_USER: AppUserRecord = {
  username: "alice",
  role: "member",
  status: "active",
  updatedAt: 200,
};

function props(overrides: Partial<SharedCasePanelProps> = {}): SharedCasePanelProps {
  return {
    open: true,
    isAdmin: false,
    currentUsername: "alice",
    cases: [CASE_RECORD],
    publications: [PUBLICATION],
    selectedCaseId: CASE_RECORD.id,
    publishGrantText: "admin: discover, preview, reference, download",
    grantDrafts: {},
    loading: false,
    busyAction: null,
    error: null,
    notice: null,
    onClose: vi.fn(),
    onRefresh: vi.fn(),
    onSelectedCaseChange: vi.fn(),
    onPublishGrantTextChange: vi.fn(),
    onPublish: vi.fn(),
    onGrantDraftChange: vi.fn(),
    onSaveGrants: vi.fn(),
    onWithdraw: vi.fn(),
    ...overrides,
  };
}

describe("shared case grants", () => {
  it("parses normalized grants and rejects unknown permissions", () => {
    expect(parseSharedCaseGrants("alice: discover, preview\nalice: reference\nbob=discover download")).toEqual([
      { username: "alice", permissions: ["discover", "preview", "reference"] },
      { username: "bob", permissions: ["discover", "download"] },
    ]);
    expect(() => parseSharedCaseGrants("alice: discover, owner")).toThrow("不支持的案例权限");
  });

  it("keeps the administrator discoverable after full grant replacement", () => {
    expect(ensureSharedCaseManagerGrant([
      { username: "alice", permissions: ["discover", "preview"] },
    ], "admin")).toEqual([
      { username: "admin", permissions: ["discover", "preview", "reference", "download"] },
      { username: "alice", permissions: ["discover", "preview"] },
    ]);
  });
});

describe("SharedCasePanel", () => {
  it("renders publishing, grant replacement, and withdrawal controls for administrators", () => {
    const html = renderToStaticMarkup(<SharedCasePanel {...props({
      isAdmin: true,
      currentUsername: "admin",
      grantDrafts: { [PUBLICATION.id]: "admin: discover, preview, reference, download" },
    })} />);

    expect(html).toContain("发布本地案例");
    expect(html).toContain("保存授权");
    expect(html).toContain("撤回");
    expect(html).toContain("直接复用案例库资产和现有 R2 备份链路");
    expect(html).toContain(CASE_RECORD.title);
  });

  it("renders only authorized discovery details for ordinary users", () => {
    const html = renderToStaticMarkup(<SharedCasePanel {...props({
      lastEventSequence: SHARED_CASE_EVENT.sequence,
    } as Partial<SharedCasePanelProps> & { lastEventSequence: number })} />);

    expect(html).toContain("已授权案例");
    expect(html).toContain("可发现");
    expect(html).toContain("可预览");
    expect(html).toContain("可引用");
    expect(html).not.toContain("发布本地案例");
    expect(html).not.toContain("保存授权");
    expect(html).not.toContain("撤回");
    expect(html).not.toContain("管理事件序列");
  });

  it("shows the last durable event sequence to administrators", () => {
    const html = renderToStaticMarkup(<SharedCasePanel {...props({
      isAdmin: true,
      currentUsername: "admin",
      lastEventSequence: SHARED_CASE_EVENT.sequence,
    } as Partial<SharedCasePanelProps> & { lastEventSequence: number })} />);

    expect(html).toContain(`管理事件序列：#${SHARED_CASE_EVENT.sequence}`);
  });
});

describe("SharedCaseCenter role boundaries", () => {
  it("does not replay administrator events for ordinary users", async () => {
    const replaySharedCaseEvents = vi.fn().mockResolvedValue([SHARED_CASE_EVENT]);
    const harness = await createSharedCaseCenterHarness(MEMBER_USER, {
      listAuthorizedSharedCases: vi.fn().mockResolvedValue([PUBLICATION]),
      refreshCases: vi.fn().mockResolvedValue([CASE_RECORD]),
      replaySharedCaseEvents,
    });

    try {
      harness.open();
      harness.runEffects();
      await flushPromises();

      expect(replaySharedCaseEvents).not.toHaveBeenCalled();
      const html = renderToStaticMarkup(harness.render());
      expect(html).not.toContain("管理事件序列");
      expect(html).not.toContain("发布本地案例");
      expect(html).not.toContain("保存授权");
      expect(html).not.toContain("撤回");
    } finally {
      harness.dispose();
    }
  });

  it("replays durable events for administrators and renders the last sequence", async () => {
    const replaySharedCaseEvents = vi.fn().mockResolvedValue([SHARED_CASE_EVENT]);
    const harness = await createSharedCaseCenterHarness(ADMIN_USER, {
      listAuthorizedSharedCases: vi.fn().mockResolvedValue([PUBLICATION]),
      refreshCases: vi.fn().mockResolvedValue([CASE_RECORD]),
      replaySharedCaseEvents,
    });

    try {
      harness.open();
      harness.runEffects();
      await flushPromises();

      expect(replaySharedCaseEvents).toHaveBeenCalledTimes(1);
      expect(renderToStaticMarkup(harness.render())).toContain(`管理事件序列：#${SHARED_CASE_EVENT.sequence}`);
    } finally {
      harness.dispose();
    }
  });
});

describe("shared case publish candidates", () => {
  it("keeps only current-project cases without an active publication", () => {
    type CandidateSelector = (
      cases: readonly CaseRecord[],
      publications: readonly SharedCasePublicationRecord[],
      activeProjectId: string | null,
    ) => readonly CaseRecord[];
    const filterPublishableSharedCaseCandidates = (
      SharedCaseCenterModule as typeof SharedCaseCenterModule & {
        filterPublishableSharedCaseCandidates?: CandidateSelector;
      }
    ).filterPublishableSharedCaseCandidates;
    expect(
      filterPublishableSharedCaseCandidates,
      "SharedCaseCenter.tsx 需要导出 filterPublishableSharedCaseCandidates(cases, publications, activeProjectId)",
    ).toBeTypeOf("function");
    if (!filterPublishableSharedCaseCandidates) return;

    const unpublishedCurrentCase = caseRecord("case-current-new", "asset-current-new", "project-1");
    const withdrawnCurrentCase = caseRecord("case-current-withdrawn", "asset-current-withdrawn", "project-1");
    const pendingCurrentCase = caseRecord("case-current-pending", "asset-current-pending", "project-1");
    const otherProjectCase = caseRecord("case-other", "asset-other", "project-2");
    const publications: SharedCasePublicationRecord[] = [
      PUBLICATION,
      publicationFor(pendingCurrentCase, "pendingBackup"),
      publicationFor(withdrawnCurrentCase, "withdrawn"),
    ];

    expect(filterPublishableSharedCaseCandidates(
      [CASE_RECORD, unpublishedCurrentCase, withdrawnCurrentCase, pendingCurrentCase, otherProjectCase],
      publications,
      "project-1",
    ).map((candidate) => candidate.id)).toEqual([
      unpublishedCurrentCase.id,
      withdrawnCurrentCase.id,
    ]);
  });
});

function caseRecord(id: string, assetId: string, projectId: string): CaseRecord {
  return {
    ...CASE_RECORD,
    id,
    assetId,
    projectId,
    title: id,
  };
}

function publicationFor(
  caseRecordValue: CaseRecord,
  status: SharedCasePublicationRecord["status"],
): SharedCasePublicationRecord {
  return {
    ...PUBLICATION,
    id: `publication-${caseRecordValue.id}`,
    caseId: caseRecordValue.id,
    assetId: caseRecordValue.assetId,
    projectId: caseRecordValue.projectId,
    title: caseRecordValue.title,
    status,
    withdrawnAt: status === "withdrawn" ? 300 : null,
  };
}

async function createSharedCaseCenterHarness(
  currentUser: AppUserRecord,
  clientMethods: Pick<BsaigcClient, "listAuthorizedSharedCases" | "refreshCases" | "replaySharedCaseEvents">,
) {
  const states: unknown[] = [];
  const effects: Array<() => void | (() => void)> = [];
  let cursor = 0;
  const useState = <Value,>(initialValue: Value | (() => Value)) => {
    const stateIndex = cursor;
    cursor += 1;
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
  const useEffect = (effect: () => void | (() => void)) => {
    effects.push(effect);
  };

  vi.resetModules();
  vi.doMock("react", async () => {
    const actual = await vi.importActual<typeof import("react")>("react");
    return { ...actual, useState, useEffect };
  });
  const { SharedCaseCenter } = await import("./SharedCaseCenter");
  const client = clientMethods as BsaigcClient;
  const render = () => {
    cursor = 0;
    effects.length = 0;
    return SharedCaseCenter({ client, currentUser, activeProjectId: "project-1" });
  };

  render();
  return {
    open() {
      states[0] = true;
      render();
    },
    runEffects() {
      for (const effect of [...effects]) effect();
    },
    render,
    dispose() {
      vi.doUnmock("react");
      vi.resetModules();
    },
  };
}

async function flushPromises(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}
