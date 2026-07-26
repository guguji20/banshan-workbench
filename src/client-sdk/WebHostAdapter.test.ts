import { describe, expect, it, vi } from "vitest";
import type { BackupCommandEnvelope } from "../generated/bsaigc/BackupCommandEnvelope";
import type { AiCredentialCommandEnvelope } from "../generated/bsaigc/AiCredentialCommandEnvelope";
import type { ContractReviewCommandEnvelope } from "../generated/bsaigc/ContractReviewCommandEnvelope";
import { BSAIGC_PROTOCOL_VERSION } from "./HostAdapter";
import { WebHostAdapter } from "./WebHostAdapter";

const context = {
  actorId: "operator-1",
  accountId: null,
  projectId: "project-1",
  windowId: "business-workbench",
  traceId: "trace-1",
};

const contractCommand: ContractReviewCommandEnvelope = {
  commandType: "contractReview.create",
  commandId: "contract-command-1",
  protocolVersion: BSAIGC_PROTOCOL_VERSION,
  context,
  payload: { workspaceId: "workspace-1", sourceAssetId: "asset-contract" },
  idempotencyKey: "contract-idempotency-1",
  expectedRevision: null,
  deadlineAt: 10_000,
};

const aiCredentialCommand: AiCredentialCommandEnvelope = {
  commandType: "aiCredentials.status",
  commandId: "ai-credential-command-1",
  protocolVersion: BSAIGC_PROTOCOL_VERSION,
  context,
  idempotencyKey: "ai-credential-idempotency-1",
  expectedRevision: null,
  deadlineAt: 10_000,
};

const backupCommand: BackupCommandEnvelope = {
  commandType: "backup.restore",
  commandId: "backup-restore-command-1",
  protocolVersion: BSAIGC_PROTOCOL_VERSION,
  context,
  payload: {
    assetId: "asset-contract",
    expectedSha256: "a".repeat(64),
  },
  idempotencyKey: "backup-restore-idempotency-1",
  expectedRevision: 7,
  deadlineAt: 10_000,
};

const unsupported = {
  code: "NOT_CONFIGURED",
  message:
    "WebHostAdapter only reserves the HTTPS/WebSocket protocol mapping and is not implemented. Use DesktopHostAdapter.",
  retryable: false,
};

describe("WebHostAdapter contract review and backup reservation", () => {
  it("keeps AI credential commands as a protocol-only placeholder", async () => {
    await expect(
      new WebHostAdapter().executeAiCredentialCommand(aiCredentialCommand),
    ).rejects.toEqual(unsupported);
  });

  it("keeps every contract review operation explicitly unsupported", async () => {
    const adapter = new WebHostAdapter();
    const listener = vi.fn();
    const operations = [
      adapter.executeContractReviewCommand(contractCommand),
      adapter.listContractReviews({ workspaceId: null, status: null, limit: null }),
      adapter.getContractReview({ reviewId: "review-1" }),
      adapter.listReviewFindings({ reviewId: "review-1", status: null }),
      adapter.getEvidenceContext({ evidenceId: "evidence-1" }),
      adapter.replayContractReviewEvents(0, 50),
      adapter.subscribeContractReviewEvents(listener),
    ];

    const results = await Promise.allSettled(operations);
    expect(results).toHaveLength(7);
    for (const result of results) {
      expect(result).toEqual({ status: "rejected", reason: unsupported });
    }
    expect(listener).not.toHaveBeenCalled();
  });


  it("keeps file actions behind stable asset IDs without exposing paths", async () => {
    const adapter = new WebHostAdapter();

    await expect(adapter.getAssetActionCapabilities("asset-report")).resolves.toEqual({
      assetId: "asset-report",
      canOpen: false,
      canExport: false,
      reason: "网页版暂未启用文件打开和导出能力。",
    });
    await expect(adapter.openAsset("asset-report")).rejects.toEqual(unsupported);
    await expect(adapter.exportAsset("asset-report")).rejects.toEqual(unsupported);
  });

  it("keeps every backup operation explicitly unsupported", async () => {
    const adapter = new WebHostAdapter();
    const listener = vi.fn();
    const operations = [
      adapter.executeBackupCommand(backupCommand),
      adapter.listAssetBackups(50),
      adapter.replayBackupEvents(0, 50),
      adapter.subscribeBackupEvents(listener),
    ];

    const results = await Promise.allSettled(operations);
    expect(results).toHaveLength(4);
    for (const result of results) {
      expect(result).toEqual({ status: "rejected", reason: unsupported });
    }
    expect(listener).not.toHaveBeenCalled();
  });
});
