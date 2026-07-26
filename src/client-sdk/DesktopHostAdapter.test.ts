import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BackupCommandEnvelope } from "../generated/bsaigc/BackupCommandEnvelope";
import type { AiCredentialCommandEnvelope } from "../generated/bsaigc/AiCredentialCommandEnvelope";
import type { BackupDomainEvent } from "../generated/bsaigc/BackupDomainEvent";
import type { ContractReviewCommandEnvelope } from "../generated/bsaigc/ContractReviewCommandEnvelope";
import type { ContractReviewDomainEvent } from "../generated/bsaigc/ContractReviewDomainEvent";
import { DesktopHostAdapter } from "./DesktopHostAdapter";
import {
  BACKUP_EVENT_CHANNEL,
  BSAIGC_PROTOCOL_VERSION,
  CONTRACT_REVIEW_EVENT_CHANNEL,
} from "./HostAdapter";

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  unlisten: vi.fn(),
  subscriptions: [] as Array<{
    channel: string;
    handler: (event: { payload: unknown }) => void;
  }>,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: tauriMocks.listen,
}));

const context = {
  actorId: "operator-1",
  accountId: "agency-1",
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

describe("DesktopHostAdapter contract review and backup wiring", () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset().mockResolvedValue(undefined);
    tauriMocks.listen.mockReset().mockImplementation(
      async (
        channel: string,
        handler: (event: { payload: unknown }) => void,
      ) => {
        tauriMocks.subscriptions.push({ channel, handler });
        return tauriMocks.unlisten;
      },
    );
    tauriMocks.unlisten.mockReset();
    tauriMocks.subscriptions.length = 0;
  });

  it("maps all contract review and backup operations to typed Tauri invoke calls", async () => {
    const adapter = new DesktopHostAdapter();
    const listRequest = {
      workspaceId: "workspace-1",
      status: "completed" as const,
      limit: 25,
    };
    const findingsRequest = {
      reviewId: "review-1",
      status: "open" as const,
    };

    await adapter.executeContractReviewCommand(contractCommand);
    await adapter.listContractReviews(listRequest);
    await adapter.getContractReview({ reviewId: "review-1" });
    await adapter.listReviewFindings(findingsRequest);
    await adapter.getEvidenceContext({ evidenceId: "evidence-1" });
    await adapter.replayContractReviewEvents(11, 50);
    await adapter.executeAiCredentialCommand(aiCredentialCommand);
    await adapter.executeBackupCommand(backupCommand);
    await adapter.listAssetBackups(75);
    await adapter.replayBackupEvents(21, 100);
    await adapter.getAssetActionCapabilities("asset-report");
    await adapter.openAsset("asset-report");
    await adapter.exportAsset("asset-report");

    expect(tauriMocks.invoke.mock.calls).toEqual([
      ["execute_contract_review_command", { command: contractCommand }],
      ["list_contract_reviews", { request: listRequest }],
      ["get_contract_review", { request: { reviewId: "review-1" } }],
      ["list_review_findings", { request: findingsRequest }],
      ["get_evidence_context", { request: { evidenceId: "evidence-1" } }],
      [
        "replay_contract_review_events",
        { request: { afterSequence: 11, limit: 50 } },
      ],
      ["execute_ai_credential_command", { command: aiCredentialCommand }],
      ["execute_backup_command", { command: backupCommand }],
      ["list_asset_backups", { limit: 75 }],
      ["replay_backup_events", { request: { afterSequence: 21, limit: 100 } }],
      ["get_asset_action_capabilities", { assetId: "asset-report" }],
      ["open_asset", { assetId: "asset-report" }],
      ["export_asset", { assetId: "asset-report" }],
    ]);
  });

  it("uses frozen event channels and forwards only serialized event payloads", async () => {
    const adapter = new DesktopHostAdapter();
    const contractListener = vi.fn();
    const backupListener = vi.fn();

    const unsubscribeContract = await adapter.subscribeContractReviewEvents(
      contractListener,
    );
    const unsubscribeBackup = await adapter.subscribeBackupEvents(backupListener);

    expect(CONTRACT_REVIEW_EVENT_CHANNEL).toBe(
      "bsaigc://contract-review-event",
    );
    expect(BACKUP_EVENT_CHANNEL).toBe("bsaigc://backup-event");
    expect(tauriMocks.listen).toHaveBeenNthCalledWith(
      1,
      CONTRACT_REVIEW_EVENT_CHANNEL,
      expect.any(Function),
    );
    expect(tauriMocks.listen).toHaveBeenNthCalledWith(
      2,
      BACKUP_EVENT_CHANNEL,
      expect.any(Function),
    );

    const contractEvent = { sequence: 1 } as ContractReviewDomainEvent;
    const backupEvent = {
      sequence: 2,
      eventType: "backup.restored",
    } as BackupDomainEvent;
    tauriMocks.subscriptions[0].handler({ payload: contractEvent });
    tauriMocks.subscriptions[1].handler({ payload: backupEvent });

    expect(contractListener).toHaveBeenCalledWith(contractEvent);
    expect(backupListener).toHaveBeenCalledWith(backupEvent);
    unsubscribeContract();
    unsubscribeBackup();
    expect(tauriMocks.unlisten).toHaveBeenCalledTimes(2);
  });
});
