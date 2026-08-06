[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repoRoot
try {
    $env:CARGO_BUILD_JOBS = "1"
    $env:CARGO_PROFILE_TEST_DEBUG = "0"
    $env:CARGO_INCREMENTAL = "0"
    & cargo test --manifest-path "src-tauri/Cargo.toml" "protocol::export_bindings"
    if ($LASTEXITCODE -ne 0) {
        Write-Error "BSAIGC protocol generation failed with exit code $LASTEXITCODE."
        exit $LASTEXITCODE
    }

    $requiredBindings = @(
        "src/generated/bsaigc/CommandEnvelope.ts",
        "src/generated/bsaigc/DomainEvent.ts",
        "src/generated/bsaigc/ProjectRecord.ts",
        "src/generated/bsaigc/TaskCommandEnvelope.ts",
        "src/generated/bsaigc/TaskRecord.ts",
        "src/generated/bsaigc/AssetCommandEnvelope.ts",
        "src/generated/bsaigc/AssetRecord.ts",
        "src/generated/bsaigc/BrainAccessMode.ts",
        "src/generated/bsaigc/BrainAttachmentPreview.ts",
        "src/generated/bsaigc/BrainDroppedItems.ts",
        "src/generated/bsaigc/BrainTurnContext.ts",
        "src/generated/bsaigc/BrainWorkspaceSelection.ts",
        "src/generated/bsaigc/StageClipboardImageRequest.ts",
        "src/generated/bsaigc/CaseCommandEnvelope.ts",
        "src/generated/bsaigc/CaseCommandResponse.ts",
        "src/generated/bsaigc/CaseDomainEvent.ts",
        "src/generated/bsaigc/CaseRecord.ts",
        "src/generated/bsaigc/SharedCaseCommandEnvelope.ts",
        "src/generated/bsaigc/SharedCaseCommandResponse.ts",
        "src/generated/bsaigc/SharedCaseDomainEvent.ts",
        "src/generated/bsaigc/SharedCaseEventType.ts",
        "src/generated/bsaigc/SharedCaseGrant.ts",
        "src/generated/bsaigc/SharedCasePermission.ts",
        "src/generated/bsaigc/SharedCasePublicationRecord.ts",
        "src/generated/bsaigc/SharedCasePublicationStatus.ts",
        "src/generated/bsaigc/ExecutionBriefCommandEnvelope.ts",
        "src/generated/bsaigc/ExecutionBriefCommandResponse.ts",
        "src/generated/bsaigc/ExecutionBriefDomainEvent.ts",
        "src/generated/bsaigc/ExecutionBriefRecord.ts",
        "src/generated/bsaigc/RequirementBriefCommandEnvelope.ts",
        "src/generated/bsaigc/RequirementBriefCommandResponse.ts",
        "src/generated/bsaigc/RequirementBriefDomainEvent.ts",
        "src/generated/bsaigc/RequirementBriefRecord.ts",
        "src/generated/bsaigc/BusinessDocumentFormat.ts",
        "src/generated/bsaigc/BusinessDocumentKind.ts",
        "src/generated/bsaigc/BusinessDocumentRecord.ts",
        "src/generated/bsaigc/BusinessDocumentSnapshot.ts",
        "src/generated/bsaigc/BusinessVideoCompletionAcceptanceAssetReference.ts",
        "src/generated/bsaigc/BusinessVideoCompletionAcceptanceScreenshot.ts",
        "src/generated/bsaigc/BusinessVideoCompletionAcceptanceVideo.ts",
        "src/generated/bsaigc/BusinessVideoCompletionAcceptanceDeliveryGroup.ts",
        "src/generated/bsaigc/BusinessVideoCompletionAcceptanceData.ts",
        "src/generated/bsaigc/BusinessProductionResultConfirmationAssetReference.ts",
        "src/generated/bsaigc/BusinessProductionResultConfirmationShot.ts",
        "src/generated/bsaigc/BusinessProductionResultConfirmationStoryboard.ts",
        "src/generated/bsaigc/BusinessProductionResultConfirmationDeliveryItem.ts",
        "src/generated/bsaigc/BusinessProductionResultConfirmationData.ts",
        "src/generated/bsaigc/BusinessDocumentStatus.ts",
        "src/generated/bsaigc/BusinessLineItem.ts",
        "src/generated/bsaigc/BusinessLineItemInput.ts",
        "src/generated/bsaigc/BusinessPaymentInput.ts",
        "src/generated/bsaigc/BusinessPaymentRecord.ts",
        "src/generated/bsaigc/BusinessPaymentStatus.ts",
        "src/generated/bsaigc/BusinessProfile.ts",
        "src/generated/bsaigc/BusinessProfileInput.ts",
        "src/generated/bsaigc/BusinessWorkspaceCommandEnvelope.ts",
        "src/generated/bsaigc/BusinessWorkspaceCommandResponse.ts",
        "src/generated/bsaigc/BusinessWorkspaceDomainEvent.ts",
        "src/generated/bsaigc/BusinessWorkspaceEventType.ts",
        "src/generated/bsaigc/BusinessWorkspaceRecord.ts",
        "src/generated/bsaigc/BusinessWorkspaceStatus.ts",
        "src/generated/bsaigc/BusinessCustomerStatus.ts",
        "src/generated/bsaigc/BusinessCustomerRecord.ts",
        "src/generated/bsaigc/BusinessCustomerInput.ts",
        "src/generated/bsaigc/BusinessArtifactRef.ts",
        "src/generated/bsaigc/BusinessMilestoneStatus.ts",
        "src/generated/bsaigc/BusinessDeliverableVersionStatus.ts",
        "src/generated/bsaigc/BusinessDeliverySubmissionStatus.ts",
        "src/generated/bsaigc/BusinessDeliverableVersionRecord.ts",
        "src/generated/bsaigc/BusinessDeliverableRecord.ts",
        "src/generated/bsaigc/BusinessMilestoneRecord.ts",
        "src/generated/bsaigc/BusinessDeliverySignoffRecord.ts",
        "src/generated/bsaigc/BusinessDeliverySubmissionRecord.ts",
        "src/generated/bsaigc/BusinessInvoiceKind.ts",
        "src/generated/bsaigc/BusinessInvoiceStatus.ts",
        "src/generated/bsaigc/BusinessInvoiceRecord.ts",
        "src/generated/bsaigc/BusinessArchiveIntegrityStatus.ts",
        "src/generated/bsaigc/BusinessArchiveEntryRecord.ts",
        "src/generated/bsaigc/BusinessArchiveSnapshotRecord.ts",
        "src/generated/bsaigc/ListBusinessCustomersRequest.ts",
        "src/generated/bsaigc/BusinessCustomerReceivableSummary.ts",
        "src/generated/bsaigc/UpsertBusinessCustomerPayload.ts",
        "src/generated/bsaigc/AssignBusinessCustomerPayload.ts",
        "src/generated/bsaigc/BusinessMilestoneInput.ts",
        "src/generated/bsaigc/UpsertBusinessMilestonePayload.ts",
        "src/generated/bsaigc/BusinessAcceptanceMaterialKind.ts",
        "src/generated/bsaigc/BusinessContractSettlementData.ts",
        "src/generated/bsaigc/BusinessServiceSettlementItemData.ts",
        "src/generated/bsaigc/BusinessAcceptanceBatchStatus.ts",
        "src/generated/bsaigc/BusinessAcceptanceRequirementRecord.ts",
        "src/generated/bsaigc/BusinessAcceptanceOutputSpecRecord.ts",
        "src/generated/bsaigc/BusinessAcceptanceMaterialBinding.ts",
        "src/generated/bsaigc/BusinessAcceptanceMaterialRecord.ts",
        "src/generated/bsaigc/BusinessAcceptanceBlocker.ts",
        "src/generated/bsaigc/BusinessAcceptanceReadiness.ts",
        "src/generated/bsaigc/BusinessAcceptanceBatchRecord.ts",
        "src/generated/bsaigc/BusinessAcceptanceRequirementInput.ts",
        "src/generated/bsaigc/BusinessAcceptanceOutputSpecInput.ts",
        "src/generated/bsaigc/BusinessAcceptanceMaterialInput.ts",
        "src/generated/bsaigc/CreateBusinessAcceptanceBatchPayload.ts",
        "src/generated/bsaigc/PrepareBusinessAcceptanceDocumentsPayload.ts",
        "src/generated/bsaigc/UpsertBusinessAcceptanceMaterialPayload.ts",
        "src/generated/bsaigc/RegisterBusinessDeliverableVersionPayload.ts",
        "src/generated/bsaigc/RecordBusinessDeliverySentPayload.ts",
        "src/generated/bsaigc/RecordBusinessDeliverySignoffPayload.ts",
        "src/generated/bsaigc/RecordBusinessInvoiceIssuedPayload.ts",
        "src/generated/bsaigc/RecordBusinessInvoiceRedCorrectionPayload.ts",
        "src/generated/bsaigc/AttachBusinessInvoiceAssetPayload.ts",
        "src/generated/bsaigc/CreateBusinessArchiveSnapshotPayload.ts",
        "src/generated/bsaigc/BusinessWorkspacePrefillField.ts",
        "src/generated/bsaigc/BusinessWorkspacePrefillMatchKind.ts",
        "src/generated/bsaigc/BusinessWorkspacePrefillDecision.ts",
        "src/generated/bsaigc/BusinessWorkspacePrefillCandidate.ts",
        "src/generated/bsaigc/ListBusinessWorkspacePrefillCandidatesRequest.ts",
        "src/generated/bsaigc/PreviewBusinessWorkspacePrefillRequest.ts",
        "src/generated/bsaigc/BusinessWorkspacePrefillChange.ts",
        "src/generated/bsaigc/BusinessWorkspacePrefillPreview.ts",
        "src/generated/bsaigc/ChangeBusinessDocumentStatusPayload.ts",
        "src/generated/bsaigc/ChangeBusinessWorkspaceStatusPayload.ts",
        "src/generated/bsaigc/CreateBusinessDocumentPayload.ts",
        "src/generated/bsaigc/CreateBusinessWorkspacePayload.ts",
        "src/generated/bsaigc/GenerateBusinessDocumentPayload.ts",
        "src/generated/bsaigc/UpdateBusinessProfilePayload.ts",
        "src/generated/bsaigc/UpsertBusinessPaymentPayload.ts",
        "src/generated/bsaigc/BrainStreamEvent.ts",
        "src/generated/bsaigc/StartBrainThreadRequest.ts",
        "src/generated/bsaigc/ResumeBrainThreadRequest.ts",
        "src/generated/bsaigc/ListRemoteBrainThreadsRequest.ts",
        "src/generated/bsaigc/StartBrainTurnRequest.ts",
        "src/generated/bsaigc/InterruptBrainTurnRequest.ts",
        "src/generated/bsaigc/RemoteBrainThreadPage.ts",
        "src/generated/bsaigc/BrainTurnStartResult.ts",
        "src/generated/bsaigc/BrainHostHealth.ts",
        "src/generated/bsaigc/NativeMediaHealth.ts",
        "src/generated/bsaigc/MemoryRecord.ts",
        "src/generated/bsaigc/ModuleManifest.ts"
    )

    $missingBindings = @(
        $requiredBindings | Where-Object {
            -not (Test-Path -LiteralPath $_ -PathType Leaf) -or
            (Get-Item -LiteralPath $_).Length -eq 0
        }
    )

    if ($missingBindings.Count -gt 0) {
        Write-Error ("Protocol generation completed without required bindings: {0}" -f ($missingBindings -join ", "))
        exit 1
    }

    Write-Host "BSAIGC protocol bindings generated and verified."
}
finally {
    Pop-Location
}
