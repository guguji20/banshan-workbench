use crate::brain_host::{BrainHost, StructuredBrainTurnRequest};
use crate::codex_runtime::CancellationToken;
use crate::document_intelligence::sha256_text;
use crate::protocol::{
    BrainTurnStatus, ContractReviewRecord, DocumentBlockRecord, DocumentExtractionRecord,
    EvidenceAnchor, HostError, InterruptBrainTurnRequest, ReviewFindingDecision,
    ReviewFindingRecord, ReviewFindingSource, ReviewFindingStatus, ReviewSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const MAX_AGENT_FINDINGS: usize = 40;
const MAX_AGENT_INPUT_CHARS: usize = 92_000;
const MAX_BLOCK_TEXT_CHARS: usize = 6_000;
const CONTRACT_AGENT_TIMEOUT: Duration = Duration::from_secs(180);
const CONTRACT_AGENT_CANCEL_POLL: Duration = Duration::from_millis(50);
const CONTRACT_AGENT_INTERRUPT_RETRIES: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct ContractAgentReviewResult {
    pub thread_id: String,
    pub agent_run_id: String,
    pub findings: Vec<ReviewFindingRecord>,
    pub evidence: Vec<EvidenceAnchor>,
}

pub trait ContractAgentReviewer: Send + Sync {
    fn review(
        &self,
        review: &ContractReviewRecord,
        extraction: &DocumentExtractionRecord,
    ) -> Result<ContractAgentReviewResult, HostError>;

    fn review_with_cancellation(
        &self,
        review: &ContractReviewRecord,
        extraction: &DocumentExtractionRecord,
        cancellation: &CancellationToken,
    ) -> Result<ContractAgentReviewResult, HostError> {
        cancellation.check_cancelled()?;
        let result = self.review(review, extraction)?;
        cancellation.check_cancelled()?;
        Ok(result)
    }

    /// Only production agents that own all runtime state opt into detached
    /// execution. Test agents remain synchronous unless they explicitly clone.
    fn detached_clone(&self) -> Option<Box<dyn ContractAgentReviewer + Send + Sync>> {
        None
    }
}

fn contract_review_title(review_id: &str) -> String {
    format!("\u{5408}\u{540c}\u{5ba1}\u{67e5} \u{00b7} {review_id}")
}

#[derive(Clone)]
pub struct CodexContractAgent {
    brain: BrainHost,
}

impl CodexContractAgent {
    pub fn new(brain: &BrainHost) -> Self {
        Self {
            brain: brain.clone(),
        }
    }

    fn run_with_cancellation(
        &self,
        review: &ContractReviewRecord,
        extraction: &DocumentExtractionRecord,
        cancellation: &CancellationToken,
    ) -> Result<ContractAgentReviewResult, HostError> {
        cancellation.check_cancelled()?;
        let title = contract_review_title(&review.session.id);
        let request = StructuredBrainTurnRequest {
            project_id: Some(review.session.workspace_id.clone()),
            title: Some(title.clone()),
            input_text: build_agent_prompt(review, extraction)?,
            output_schema: contract_agent_output_schema(),
            model: None,
            effort: Some("high".to_string()),
            timeout: CONTRACT_AGENT_TIMEOUT,
        };
        let brain = self.brain.clone();
        let turn_brain = brain.clone();
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name(format!("contract-agent-{}", review.session.id))
            .spawn(move || {
                let _ = sender.send(turn_brain.run_structured_turn(request));
            })
            .map_err(|error| {
                HostError::new(
                    "CONTRACT_AGENT_THREAD_FAILED",
                    format!("unable to start Codex contract review worker: {error}"),
                    true,
                )
            })?;

        let turn = loop {
            match receiver.recv_timeout(CONTRACT_AGENT_CANCEL_POLL) {
                Ok(result) => break result?,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancellation.is_cancelled() {
                        best_effort_interrupt_contract_turn(
                            &brain,
                            &review.session.workspace_id,
                            &title,
                        );
                        return Err(HostError::new(
                            "CONTRACT_REVIEW_CANCELLED",
                            "contract review was cancelled",
                            false,
                        ));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(HostError::new(
                        "CONTRACT_AGENT_THREAD_FAILED",
                        "Codex contract review worker stopped without a result",
                        true,
                    ));
                }
            }
        };
        cancellation.check_cancelled()?;
        parse_agent_output(
            review,
            extraction,
            &turn.turn_id,
            &turn.thread_id,
            &turn.assistant_text,
            now_ms()?,
        )
    }
}

impl ContractAgentReviewer for CodexContractAgent {
    fn review(
        &self,
        review: &ContractReviewRecord,
        extraction: &DocumentExtractionRecord,
    ) -> Result<ContractAgentReviewResult, HostError> {
        self.run_with_cancellation(review, extraction, &CancellationToken::new())
    }

    fn review_with_cancellation(
        &self,
        review: &ContractReviewRecord,
        extraction: &DocumentExtractionRecord,
        cancellation: &CancellationToken,
    ) -> Result<ContractAgentReviewResult, HostError> {
        self.run_with_cancellation(review, extraction, cancellation)
    }

    fn detached_clone(&self) -> Option<Box<dyn ContractAgentReviewer + Send + Sync>> {
        Some(Box::new(self.clone()))
    }
}

fn best_effort_interrupt_contract_turn(brain: &BrainHost, project_id: &str, title: &str) {
    for _ in 0..CONTRACT_AGENT_INTERRUPT_RETRIES {
        let threads = match brain.list_local_threads(Some(project_id)) {
            Ok(threads) => threads,
            Err(_) => return,
        };
        for thread_record in threads
            .into_iter()
            .filter(|thread_record| thread_record.title.as_deref() == Some(title))
        {
            let turns = match brain.list_local_turns(&thread_record.id) {
                Ok(turns) => turns,
                Err(_) => continue,
            };
            for turn in turns
                .into_iter()
                .filter(|turn| turn.status == BrainTurnStatus::Running)
            {
                if brain
                    .interrupt_turn(InterruptBrainTurnRequest {
                        thread_id: thread_record.id.clone(),
                        turn_id: turn.id,
                    })
                    .is_ok()
                {
                    return;
                }
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInputBlock {
    id: String,
    page_index: i64,
    order_index: i64,
    kind: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentReviewOutput {
    findings: Vec<AgentFindingDraft>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentFindingDraft {
    category: String,
    severity: ReviewSeverity,
    title: String,
    description: String,
    recommendation: String,
    evidence_block_ids: Vec<String>,
    missing_evidence_reason: Option<String>,
}

pub fn contract_agent_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["findings"],
        "properties": {
            "findings": {
                "type": "array",
                "maxItems": MAX_AGENT_FINDINGS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "category", "severity", "title", "description", "recommendation",
                        "evidenceBlockIds", "missingEvidenceReason"
                    ],
                    "properties": {
                        "category": { "type": "string", "minLength": 1, "maxLength": 120 },
                        "severity": {
                            "type": "string",
                            "enum": ["info", "low", "medium", "high", "critical"]
                        },
                        "title": { "type": "string", "minLength": 1, "maxLength": 1000 },
                        "description": { "type": "string", "minLength": 1, "maxLength": 16000 },
                        "recommendation": { "type": "string", "minLength": 1, "maxLength": 16000 },
                        "evidenceBlockIds": {
                            "type": "array",
                            "maxItems": 8,
                            "items": { "type": "string", "minLength": 1, "maxLength": 128 }
                        },
                        "missingEvidenceReason": {
                            "type": ["string", "null"],
                            "maxLength": 4000
                        }
                    }
                }
            }
        }
    })
}

fn build_agent_prompt(
    review: &ContractReviewRecord,
    extraction: &DocumentExtractionRecord,
) -> Result<String, HostError> {
    let mut blocks = Vec::new();
    let mut used_chars = 0usize;
    for block in &extraction.blocks {
        if used_chars >= MAX_AGENT_INPUT_CHARS {
            break;
        }
        let text = truncate_chars(&block.text, MAX_BLOCK_TEXT_CHARS);
        used_chars = used_chars.saturating_add(text.chars().count());
        blocks.push(AgentInputBlock {
            id: block.id.clone(),
            page_index: block.page_index,
            order_index: block.order_index,
            kind: format!("{:?}", block.kind),
            text,
        });
    }
    let deterministic_findings = review
        .findings
        .iter()
        .filter(|finding| finding.source == ReviewFindingSource::Rule)
        .map(|finding| {
            json!({
                "category": finding.category,
                "severity": finding.severity,
                "title": finding.title,
                "description": finding.description,
                "missingEvidenceReason": finding.missing_evidence_reason,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "reviewId": review.session.id,
        "sourceFileName": review.session.source_file_name,
        "sourceAssetSha256": review.session.source_asset_sha256,
        "parser": extraction.parser,
        "blocks": blocks,
        "deterministicFindings": deterministic_findings,
        "truncated": extraction.blocks.len() > blocks.len(),
    });
    let payload = serde_json::to_string(&payload).map_err(|error| {
        HostError::internal(format!("serialize contract Agent input failed: {error}"))
    })?;
    let prompt = format!(
        "你是中国大陆影视制作与营销服务合同的高级商务审查 Agent。\n\
         只审查输入 JSON 中的合同，不使用外部事实，不执行工具，不修改文件。\n\
         重点检查主体、金额与税费、付款条件、发票、服务范围、交付物、验收、修改轮次、工期、知识产权、肖像/素材授权、保密、违约、解约、不可抗力、争议解决和条款冲突。\n\
         确定性规则已发现的同类问题不要重复；只补充规则未覆盖、上下文矛盾或需要专业判断的风险。\n\
         每项 finding 必须引用一个或多个输入 block id。只有合同确实缺少相关条款时，evidenceBlockIds 才能为空，并必须填写 missingEvidenceReason。\n\
         严禁编造 block id、金额、日期、主体或合同原文。没有额外风险时返回空 findings。\n\nCONTRACT_INPUT_JSON:\n{payload}"
    );
    if prompt.chars().count() > 100_000 {
        return Err(HostError::new(
            "CONTRACT_AGENT_INPUT_TOO_LARGE",
            "contract Agent input exceeds the managed Brain limit",
            false,
        ));
    }
    Ok(prompt)
}

fn parse_agent_output(
    review: &ContractReviewRecord,
    extraction: &DocumentExtractionRecord,
    agent_run_id: &str,
    thread_id: &str,
    assistant_text: &str,
    now: i64,
) -> Result<ContractAgentReviewResult, HostError> {
    let output: AgentReviewOutput = serde_json::from_str(assistant_text).map_err(|error| {
        HostError::new(
            "CONTRACT_AGENT_OUTPUT_INVALID",
            format!("Codex contract-review output is not valid structured JSON: {error}"),
            true,
        )
    })?;
    if output.findings.len() > MAX_AGENT_FINDINGS {
        return Err(HostError::new(
            "CONTRACT_AGENT_OUTPUT_INVALID",
            "Codex contract-review output exceeds the finding limit",
            true,
        ));
    }
    let blocks = extraction
        .blocks
        .iter()
        .map(|block| (block.id.as_str(), block))
        .collect::<HashMap<_, _>>();
    let mut evidence_by_block = BTreeMap::<String, EvidenceAnchor>::new();
    let mut findings = Vec::with_capacity(output.findings.len());
    let mut semantic_keys = HashSet::new();
    for (index, draft) in output.findings.into_iter().enumerate() {
        validate_draft(&draft)?;
        let semantic_key = format!(
            "{}\u{1f}{}",
            draft.category.trim().to_lowercase(),
            draft.title.trim().to_lowercase()
        );
        if !semantic_keys.insert(semantic_key) {
            continue;
        }
        let mut evidence_ids = Vec::new();
        let mut seen_blocks = HashSet::new();
        for block_id in draft.evidence_block_ids {
            if !seen_blocks.insert(block_id.clone()) {
                continue;
            }
            let block = blocks.get(block_id.as_str()).ok_or_else(|| {
                HostError::new(
                    "CONTRACT_AGENT_EVIDENCE_INVALID",
                    format!("Codex referenced unknown extraction block {block_id}"),
                    true,
                )
            })?;
            let anchor = evidence_by_block
                .entry(block_id.clone())
                .or_insert_with(|| evidence_from_block(review, extraction, block));
            evidence_ids.push(anchor.id.clone());
        }
        let missing_evidence_reason = draft
            .missing_evidence_reason
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if evidence_ids.is_empty() && missing_evidence_reason.is_none() {
            return Err(HostError::new(
                "CONTRACT_AGENT_EVIDENCE_REQUIRED",
                "Codex finding has neither verified evidence nor a missing-evidence reason",
                true,
            ));
        }
        let finding_id = stable_uuid(&format!(
            "agent-finding:{}:{agent_run_id}:{index}:{}:{}",
            review.session.id, draft.category, draft.title
        ));
        findings.push(ReviewFindingRecord {
            id: finding_id,
            review_id: review.session.id.clone(),
            source: ReviewFindingSource::Agent,
            rule_id: None,
            rule_version: None,
            agent_run_id: Some(agent_run_id.to_string()),
            category: draft.category.trim().to_string(),
            severity: draft.severity,
            title: draft.title.trim().to_string(),
            description: draft.description.trim().to_string(),
            recommendation: draft.recommendation.trim().to_string(),
            evidence_ids,
            missing_evidence_reason,
            status: ReviewFindingStatus::Open,
            decision: ReviewFindingDecision::Unreviewed,
            revision: 1,
            created_at: now,
            updated_at: now,
        });
    }
    Ok(ContractAgentReviewResult {
        thread_id: thread_id.to_string(),
        agent_run_id: agent_run_id.to_string(),
        findings,
        evidence: evidence_by_block.into_values().collect(),
    })
}

fn evidence_from_block(
    review: &ContractReviewRecord,
    extraction: &DocumentExtractionRecord,
    block: &DocumentBlockRecord,
) -> EvidenceAnchor {
    let quoted_text = block.text.clone();
    EvidenceAnchor {
        id: stable_uuid(&format!(
            "agent-evidence:{}:{}:{}",
            review.session.id, extraction.id, block.id
        )),
        extraction_id: extraction.id.clone(),
        source_asset_id: extraction.source_asset_id.clone(),
        page_index: block.page_index,
        block_id: Some(block.id.clone()),
        char_start: Some(block.char_start),
        char_end: Some(block.char_end),
        bbox: block.bbox.clone(),
        quoted_text_sha256: sha256_text(&quoted_text),
        quoted_text,
        context_before: String::new(),
        context_after: String::new(),
    }
}

fn validate_draft(draft: &AgentFindingDraft) -> Result<(), HostError> {
    for (name, value, limit) in [
        ("category", draft.category.as_str(), 120usize),
        ("title", draft.title.as_str(), 1_000usize),
        ("description", draft.description.as_str(), 16_000usize),
        ("recommendation", draft.recommendation.as_str(), 16_000usize),
    ] {
        let count = value.trim().chars().count();
        if count == 0 || count > limit {
            return Err(HostError::new(
                "CONTRACT_AGENT_OUTPUT_INVALID",
                format!("Codex finding {name} is empty or exceeds {limit} characters"),
                true,
            ));
        }
    }
    if draft.evidence_block_ids.len() > 8 {
        return Err(HostError::new(
            "CONTRACT_AGENT_OUTPUT_INVALID",
            "Codex finding references more than eight evidence blocks",
            true,
        ));
    }
    Ok(())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn stable_uuid(seed: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes()).to_string()
}

fn now_ms() -> Result<i64, HostError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|error| HostError::internal(format!("system clock error: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_review_rules::ContractReviewRuleEngine;
    use crate::document_intelligence::build_extraction_from_text;
    use crate::protocol::{ContractReviewSessionRecord, ContractReviewStage, ContractReviewStatus};

    fn fixture() -> (ContractReviewRecord, DocumentExtractionRecord) {
        let review_id = Uuid::new_v4().to_string();
        let extraction = build_extraction_from_text(
            &review_id,
            &Uuid::new_v4().to_string(),
            &"a".repeat(64),
            "fixture",
            "1",
            "甲方：测试客户\n乙方：半山影像\n付款：验收后十日内支付。",
            1,
        );
        let rule_output = ContractReviewRuleEngine.evaluate(&review_id, &extraction, 1);
        (
            ContractReviewRecord {
                session: ContractReviewSessionRecord {
                    id: review_id,
                    workspace_id: Uuid::new_v4().to_string(),
                    source_asset_id: extraction.source_asset_id.clone(),
                    source_asset_sha256: extraction.source_asset_sha256.clone(),
                    source_file_name: "contract.docx".to_string(),
                    status: ContractReviewStatus::Running,
                    stage: ContractReviewStage::ReviewingAgent,
                    extraction_id: Some(extraction.id.clone()),
                    report_asset_id: None,
                    revision: 3,
                    created_at: 1,
                    updated_at: 1,
                    completed_at: None,
                    failure: None,
                },
                extraction: Some(extraction.clone()),
                evidence: rule_output.evidence,
                rule_evaluations: rule_output.evaluations,
                findings: rule_output.findings,
                decisions: Vec::new(),
                reports: Vec::new(),
            },
            extraction,
        )
    }

    #[test]
    fn contract_review_thread_title_is_human_readable_chinese() {
        assert_eq!(contract_review_title("review-123"), "合同审查 · review-123");
    }

    #[test]
    fn schema_is_strict_and_bounded() {
        let schema = contract_agent_output_schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["findings"]["maxItems"], 40);
        assert_eq!(
            schema["properties"]["findings"]["items"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn parses_verified_block_evidence_into_stable_records() {
        let (review, extraction) = fixture();
        let block_id = extraction.blocks[0].id.clone();
        let payload = json!({
            "findings": [{
                "category": "payment",
                "severity": "high",
                "title": "付款前置条件不完整",
                "description": "付款未绑定合法有效发票。",
                "recommendation": "增加发票前置条件。",
                "evidenceBlockIds": [block_id],
                "missingEvidenceReason": null
            }]
        })
        .to_string();
        let first =
            parse_agent_output(&review, &extraction, "run-1", "thread-1", &payload, 123).unwrap();
        let second =
            parse_agent_output(&review, &extraction, "run-1", "thread-1", &payload, 123).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.findings.len(), 1);
        assert_eq!(first.evidence.len(), 1);
        assert_eq!(first.findings[0].source, ReviewFindingSource::Agent);
        assert_eq!(
            first.findings[0].evidence_ids,
            vec![first.evidence[0].id.clone()]
        );
    }

    #[test]
    fn rejects_unknown_or_missing_evidence() {
        let (review, extraction) = fixture();
        let unknown = json!({
            "findings": [{
                "category": "scope", "severity": "medium", "title": "风险",
                "description": "描述", "recommendation": "建议",
                "evidenceBlockIds": [Uuid::new_v4().to_string()],
                "missingEvidenceReason": null
            }]
        })
        .to_string();
        assert_eq!(
            parse_agent_output(&review, &extraction, "run-1", "thread-1", &unknown, 123)
                .unwrap_err()
                .code,
            "CONTRACT_AGENT_EVIDENCE_INVALID"
        );

        let missing = json!({
            "findings": [{
                "category": "scope", "severity": "medium", "title": "风险",
                "description": "描述", "recommendation": "建议",
                "evidenceBlockIds": [], "missingEvidenceReason": null
            }]
        })
        .to_string();
        assert_eq!(
            parse_agent_output(&review, &extraction, "run-1", "thread-1", &missing, 123)
                .unwrap_err()
                .code,
            "CONTRACT_AGENT_EVIDENCE_REQUIRED"
        );
    }
}
