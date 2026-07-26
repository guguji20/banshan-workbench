import type { BusinessWorkspaceRecord } from "./generated/bsaigc/BusinessWorkspaceRecord";

export function businessWorkspaceAssetIds(
  workspace: BusinessWorkspaceRecord | null,
): string[] {
  if (!workspace) return [];
  const assetIds = new Set<string>();
  for (const document of workspace.documents) {
    if (document.outputAssetId) assetIds.add(document.outputAssetId);
    if (document.reportAssetId) assetIds.add(document.reportAssetId);
  }
  for (const milestone of workspace.milestones) {
    for (const deliverable of milestone.deliverables) {
      for (const version of deliverable.versions) {
        assetIds.add(version.artifact.assetId);
      }
    }
  }
  for (const submission of workspace.deliverySubmissions) {
    for (const signoff of submission.signoffs) {
      if (signoff.evidence) assetIds.add(signoff.evidence.assetId);
    }
  }
  for (const confirmation of workspace.quoteConfirmations) {
    assetIds.add(confirmation.quoteAssetId);
    assetIds.add(confirmation.evidence.assetId);
  }
  for (const receipt of workspace.receipts) {
    if (receipt.evidence) assetIds.add(receipt.evidence.assetId);
  }
  for (const invoice of workspace.invoices) {
    for (const artifact of invoice.artifacts) assetIds.add(artifact.assetId);
  }
  for (const snapshot of workspace.archiveSnapshots) {
    if (snapshot.manifestAssetId) assetIds.add(snapshot.manifestAssetId);
    if (snapshot.packageAssetId) assetIds.add(snapshot.packageAssetId);
    for (const entry of snapshot.entries) assetIds.add(entry.artifact.assetId);
  }
  return [...assetIds].sort();
}
