use crate::document_intelligence::sha256_text;
use crate::protocol::{
    DocumentBlockRecord, DocumentExtractionRecord, EvidenceAnchor, ReviewFindingDecision,
    ReviewFindingRecord, ReviewFindingSource, ReviewFindingStatus, ReviewSeverity,
    RuleEvaluationRecord, RuleEvaluationStatus,
};
use uuid::Uuid;

pub const RULE_SET_VERSION: &str = "business-contract-cn-1";

#[derive(Debug, Clone, PartialEq)]
pub struct RuleReviewOutput {
    pub evidence: Vec<EvidenceAnchor>,
    pub findings: Vec<ReviewFindingRecord>,
    pub evaluations: Vec<RuleEvaluationRecord>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContractReviewRuleEngine;

impl ContractReviewRuleEngine {
    pub fn evaluate(
        &self,
        review_id: &str,
        extraction: &DocumentExtractionRecord,
        now: i64,
    ) -> RuleReviewOutput {
        let rules = [
            RequiredClauseRule {
                id: "contract.parties.required",
                version: "1",
                category: "parties",
                severity: ReviewSeverity::Critical,
                title: "合同主体缺失",
                description: "未识别到完整的甲乙双方主体信息。",
                recommendation: "补充甲方、乙方的法定名称、统一社会信用代码、地址和联系人。",
                keywords: &["甲方", "乙方"],
                mode: MatchMode::All,
            },
            RequiredClauseRule {
                id: "contract.amount.required",
                version: "1",
                category: "amount",
                severity: ReviewSeverity::High,
                title: "合同金额缺失",
                description: "未识别到合同总价或服务费用金额。",
                recommendation: "补充含税/不含税金额、税率、币种及大写金额。",
                keywords: &["合同金额", "合同总价", "服务费", "项目费用", "人民币", "元"],
                mode: MatchMode::Any,
            },
            RequiredClauseRule {
                id: "contract.payment.required",
                version: "1",
                category: "payment",
                severity: ReviewSeverity::High,
                title: "付款条款缺失",
                description: "未识别到付款节点、比例、期限或收款条件。",
                recommendation: "补充首付款、进度款、尾款比例及发票和付款期限。",
                keywords: &["付款", "支付", "首付款", "预付款", "进度款", "尾款"],
                mode: MatchMode::Any,
            },
            RequiredClauseRule {
                id: "contract.acceptance.required",
                version: "1",
                category: "acceptance",
                severity: ReviewSeverity::High,
                title: "验收条款缺失",
                description: "未识别到验收标准、期限或确认方式。",
                recommendation: "补充验收标准、反馈期限、修改轮次和逾期视为验收通过的规则。",
                keywords: &["验收", "验收标准", "验收期限", "验收通过"],
                mode: MatchMode::Any,
            },
        ];

        let mut evidence = Vec::new();
        let mut findings = Vec::new();
        let mut evaluations = Vec::new();
        for rule in rules {
            let matches = collect_matches(extraction, rule.keywords);
            let passed = match rule.mode {
                MatchMode::Any => !matches.is_empty(),
                MatchMode::All => rule
                    .keywords
                    .iter()
                    .all(|keyword| matches.iter().any(|item| item.keyword == *keyword)),
            };
            let mut evidence_ids = Vec::new();
            if passed {
                for matched in matches.into_iter().take(3) {
                    let anchor = evidence_from_match(extraction, &matched);
                    evidence_ids.push(anchor.id.clone());
                    evidence.push(anchor);
                }
            }

            let finding_ids = if passed {
                Vec::new()
            } else {
                let finding_id = Uuid::new_v4().to_string();
                findings.push(ReviewFindingRecord {
                    id: finding_id.clone(),
                    review_id: review_id.to_string(),
                    source: ReviewFindingSource::Rule,
                    rule_id: Some(rule.id.to_string()),
                    rule_version: Some(rule.version.to_string()),
                    agent_run_id: None,
                    category: rule.category.to_string(),
                    severity: rule.severity,
                    title: rule.title.to_string(),
                    description: rule.description.to_string(),
                    recommendation: rule.recommendation.to_string(),
                    evidence_ids: Vec::new(),
                    missing_evidence_reason: Some(format!(
                        "未在合同解析文本中识别到必要关键词：{}",
                        rule.keywords.join("、")
                    )),
                    status: ReviewFindingStatus::Open,
                    decision: ReviewFindingDecision::Unreviewed,
                    revision: 1,
                    created_at: now,
                    updated_at: now,
                });
                vec![finding_id]
            };
            evaluations.push(RuleEvaluationRecord {
                id: Uuid::new_v4().to_string(),
                review_id: review_id.to_string(),
                rule_id: rule.id.to_string(),
                rule_version: rule.version.to_string(),
                status: if passed {
                    RuleEvaluationStatus::Passed
                } else {
                    RuleEvaluationStatus::Finding
                },
                finding_ids,
                details: if passed {
                    format!("已识别关键词：{}", rule.keywords.join("、"))
                } else {
                    format!("缺少关键词：{}", rule.keywords.join("、"))
                },
                evaluated_at: now,
            });
        }

        for rule in semantic_risk_rules() {
            append_semantic_risk_finding(
                review_id,
                extraction,
                now,
                rule,
                &mut evidence,
                &mut findings,
                &mut evaluations,
            );
        }

        RuleReviewOutput {
            evidence,
            findings,
            evaluations,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MatchMode {
    Any,
    All,
}

#[derive(Debug, Clone, Copy)]
struct RequiredClauseRule {
    id: &'static str,
    version: &'static str,
    category: &'static str,
    severity: ReviewSeverity,
    title: &'static str,
    description: &'static str,
    recommendation: &'static str,
    keywords: &'static [&'static str],
    mode: MatchMode,
}

#[derive(Debug, Clone, Copy)]
struct SemanticRiskRule {
    id: &'static str,
    version: &'static str,
    category: &'static str,
    severity: ReviewSeverity,
    title: &'static str,
    description: &'static str,
    recommendation: &'static str,
    evidence_phrases: &'static [&'static str],
}

fn semantic_risk_rules() -> [SemanticRiskRule; 16] {
    [
        SemanticRiskRule {
            id: "unilateral_change",
            version: "1",
            category: "scope",
            severity: ReviewSeverity::Critical,
            title: "甲方可单方变更且乙方无偿承担",
            description: "甲方可以单方扩大或改变工作范围，乙方不得调整费用或周期。",
            recommendation:
                "将范围、数量、风格和交付时间变更改为双方书面确认，并同步调整费用与周期。",
            evidence_phrases: &["甲方有权随时单方变更", "不得追加费用"],
        },
        SemanticRiskRule {
            id: "unlimited_liability",
            version: "1",
            category: "liability",
            severity: ReviewSeverity::Critical,
            title: "乙方承担无限责任和扩大损失",
            description: "赔偿范围包含间接或预期损失，且责任金额没有上限。",
            recommendation:
                "限定为可预见的直接损失，并设置不高于合同金额或合理比例的累计责任上限。",
            evidence_phrases: &["不受合同金额限制", "预期利益"],
        },
        SemanticRiskRule {
            id: "payment_terms_ambiguous",
            version: "1",
            category: "payment",
            severity: ReviewSeverity::High,
            title: "付款条件由甲方单方控制且无明确期限",
            description: "付款依赖甲方内部审批或资金安排，并排除了逾期责任。",
            recommendation: "明确付款比例、触发节点、最迟期限、发票条件和逾期责任。",
            evidence_phrases: &["资金安排允许时", "不构成逾期"],
        },
        SemanticRiskRule {
            id: "acceptance_criteria_ambiguous",
            version: "1",
            category: "acceptance",
            severity: ReviewSeverity::High,
            title: "验收标准主观且修改次数无限",
            description: "验收完全取决于甲方主观判断，反馈期限和修改轮次没有边界。",
            recommendation:
                "改为按书面需求和技术标准验收，并约定反馈期限、修改轮次和视为通过条件。",
            evidence_phrases: &["甲方主观满意", "不限次数"],
        },
        SemanticRiskRule {
            id: "ip_full_assignment_overreach",
            version: "1",
            category: "intellectualProperty",
            severity: ReviewSeverity::High,
            title: "知识产权转让范围覆盖乙方既有资产",
            description: "转让范围包含模板、方法、通用组件和乙方既有材料，且无偿、永久、不可撤销。",
            recommendation:
                "仅在款项付清后转让约定成果权利，保留乙方既有工具、模板、方法和通用技术。",
            evidence_phrases: &["乙方既有材料", "无偿、永久、不可撤销"],
        },
        SemanticRiskRule {
            id: "unfavorable_dispute_venue",
            version: "1",
            category: "dispute",
            severity: ReviewSeverity::High,
            title: "争议管辖地单方有利于甲方",
            description: "争议被限定在甲方所在地法院处理，显著增加乙方维权成本。",
            recommendation: "改为合同签订地、履行地或被告住所地有管辖权的法院。",
            evidence_phrases: &["北京市朝阳区人民法院"],
        },
        SemanticRiskRule {
            id: "customer_identity_missing",
            version: "1",
            category: "parties",
            severity: ReviewSeverity::Critical,
            title: "客户法定主体信息缺失",
            description: "甲方法定名称仍是占位内容，无法确认签约和付款主体。",
            recommendation: "补充甲方法定名称、统一社会信用代码、地址和联系人。",
            evidence_phrases: &["甲方（客户）：【未填写】"],
        },
        SemanticRiskRule {
            id: "provider_identity_missing",
            version: "1",
            category: "parties",
            severity: ReviewSeverity::Critical,
            title: "服务方法定主体信息缺失",
            description: "乙方法定名称仍是占位内容，无法确认履约和收款主体。",
            recommendation: "补充乙方法定名称、统一社会信用代码、地址和联系人。",
            evidence_phrases: &["乙方（服务方）：【未填写】"],
        },
        SemanticRiskRule {
            id: "project_scope_missing",
            version: "1",
            category: "scope",
            severity: ReviewSeverity::High,
            title: "项目名称和服务范围缺失",
            description: "项目名称与服务范围仍为占位内容，无法界定交付边界。",
            recommendation: "补充项目名称、服务范围、成果数量、时长、比例和源文件要求。",
            evidence_phrases: &["项目名称：【未填写】", "服务范围：【待补充】"],
        },
        SemanticRiskRule {
            id: "contract_amount_missing",
            version: "1",
            category: "amount",
            severity: ReviewSeverity::High,
            title: "金额、税率和币种缺失",
            description: "合同金额仍为占位内容，价税口径和费用边界不明确。",
            recommendation: "补充金额、币种、大小写金额、税率、含税口径和费用范围。",
            evidence_phrases: &["合同金额：【未填写】"],
        },
        SemanticRiskRule {
            id: "payment_schedule_missing",
            version: "1",
            category: "payment",
            severity: ReviewSeverity::High,
            title: "付款比例、节点和期限缺失",
            description: "付款安排仅约定另行协商，没有可执行的付款计划。",
            recommendation: "补充首付款、进度款、尾款比例、付款节点、期限和发票条件。",
            evidence_phrases: &["付款安排：【双方另行约定】"],
        },
        SemanticRiskRule {
            id: "delivery_deadline_missing",
            version: "1",
            category: "delivery",
            severity: ReviewSeverity::High,
            title: "交付日期和清单缺失",
            description: "交付日期仍待通知，交付格式和清单没有形成约束。",
            recommendation: "补充交付日期、成果清单、分辨率、格式、渠道和延期处理。",
            evidence_phrases: &["交付日期：【待通知】"],
        },
        SemanticRiskRule {
            id: "acceptance_terms_missing",
            version: "1",
            category: "acceptance",
            severity: ReviewSeverity::High,
            title: "验收标准、期限和修改轮次缺失",
            description: "验收标准仍未约定，反馈期限和修改边界无法执行。",
            recommendation: "补充验收标准、反馈期限、确认方式、修改轮次和视为通过条件。",
            evidence_phrases: &["验收标准：【未约定】"],
        },
        SemanticRiskRule {
            id: "breach_liability_missing",
            version: "1",
            category: "liability",
            severity: ReviewSeverity::Medium,
            title: "违约责任没有可执行约定",
            description: "违约责任仅约定另行协商，发生争议时缺少计算和责任边界。",
            recommendation: "补充违约金、损失范围、责任上限、免责事由和不可抗力处理。",
            evidence_phrases: &["违约责任：【另行协商】"],
        },
        SemanticRiskRule {
            id: "confidentiality_ip_missing",
            version: "1",
            category: "confidentialityAndIp",
            severity: ReviewSeverity::High,
            title: "保密和知识产权边界缺失",
            description: "保密期限和知识产权归属仍为占位内容。",
            recommendation: "补充保密范围、期限、例外、素材权属、成果使用范围和既有工具保留。",
            evidence_phrases: &["保密期限：【未填写】", "知识产权归属：【未填写】"],
        },
        SemanticRiskRule {
            id: "dispute_resolution_missing",
            version: "1",
            category: "dispute",
            severity: ReviewSeverity::Medium,
            title: "争议管辖缺失",
            description: "争议管辖仍为占位内容，无法确定争议解决地点。",
            recommendation: "补充适用法律、协商机制和明确的争议管辖。",
            evidence_phrases: &["争议管辖：【未填写】"],
        },
    ]
}

fn append_semantic_risk_finding(
    review_id: &str,
    extraction: &DocumentExtractionRecord,
    now: i64,
    rule: SemanticRiskRule,
    evidence: &mut Vec<EvidenceAnchor>,
    findings: &mut Vec<ReviewFindingRecord>,
    evaluations: &mut Vec<RuleEvaluationRecord>,
) {
    let matches = collect_matches(extraction, rule.evidence_phrases);
    if matches.is_empty() {
        return;
    }

    let mut evidence_ids = Vec::new();
    for matched in matches {
        let anchor = evidence_from_match(extraction, &matched);
        evidence_ids.push(anchor.id.clone());
        evidence.push(anchor);
    }
    let finding_id = Uuid::new_v4().to_string();
    findings.push(ReviewFindingRecord {
        id: finding_id.clone(),
        review_id: review_id.to_string(),
        source: ReviewFindingSource::Rule,
        rule_id: Some(rule.id.to_string()),
        rule_version: Some(rule.version.to_string()),
        agent_run_id: None,
        category: rule.category.to_string(),
        severity: rule.severity,
        title: rule.title.to_string(),
        description: rule.description.to_string(),
        recommendation: rule.recommendation.to_string(),
        evidence_ids,
        missing_evidence_reason: None,
        status: ReviewFindingStatus::Open,
        decision: ReviewFindingDecision::Unreviewed,
        revision: 0,
        created_at: now,
        updated_at: now,
    });
    evaluations.push(RuleEvaluationRecord {
        id: Uuid::new_v4().to_string(),
        review_id: review_id.to_string(),
        rule_id: rule.id.to_string(),
        rule_version: rule.version.to_string(),
        status: RuleEvaluationStatus::Finding,
        finding_ids: vec![finding_id],
        details: format!("识别到风险证据：{}", rule.evidence_phrases.join("、")),
        evaluated_at: now,
    });
}

#[derive(Debug, Clone)]
struct KeywordMatch<'a> {
    keyword: &'a str,
    block: &'a DocumentBlockRecord,
    byte_start: usize,
    byte_end: usize,
}

fn collect_matches<'a>(
    extraction: &'a DocumentExtractionRecord,
    keywords: &'a [&'a str],
) -> Vec<KeywordMatch<'a>> {
    let mut matches = Vec::new();
    for block in &extraction.blocks {
        for keyword in keywords {
            if let Some(byte_start) = block.text.find(keyword) {
                matches.push(KeywordMatch {
                    keyword,
                    block,
                    byte_start,
                    byte_end: byte_start + keyword.len(),
                });
            }
        }
    }
    matches
}

fn evidence_from_match(
    extraction: &DocumentExtractionRecord,
    matched: &KeywordMatch<'_>,
) -> EvidenceAnchor {
    let context_start = previous_char_boundary(&matched.block.text, matched.byte_start, 48);
    let context_end = next_char_boundary(&matched.block.text, matched.byte_end, 96);
    let quoted_text = matched.block.text[matched.byte_start..matched.byte_end].to_string();
    EvidenceAnchor {
        id: Uuid::new_v4().to_string(),
        extraction_id: extraction.id.clone(),
        source_asset_id: extraction.source_asset_id.clone(),
        page_index: matched.block.page_index,
        block_id: Some(matched.block.id.clone()),
        char_start: Some(matched.block.char_start + matched.byte_start as i64),
        char_end: Some(matched.block.char_start + matched.byte_end as i64),
        bbox: matched.block.bbox.clone(),
        quoted_text_sha256: sha256_text(&quoted_text),
        quoted_text,
        context_before: matched.block.text[context_start..matched.byte_start].to_string(),
        context_after: matched.block.text[matched.byte_end..context_end].to_string(),
    }
}

fn previous_char_boundary(value: &str, start: usize, max_bytes: usize) -> usize {
    let mut candidate = start.saturating_sub(max_bytes);
    while candidate < start && !value.is_char_boundary(candidate) {
        candidate += 1;
    }
    candidate
}

fn next_char_boundary(value: &str, end: usize, max_bytes: usize) -> usize {
    let mut candidate = end.saturating_add(max_bytes).min(value.len());
    while candidate > end && !value.is_char_boundary(candidate) {
        candidate -= 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_intelligence::build_extraction_from_text;

    fn hash() -> String {
        "b".repeat(64)
    }

    #[test]
    fn complete_contract_passes_first_rule_set_and_keeps_evidence() {
        let extraction = build_extraction_from_text(
            "review-1",
            "asset-1",
            &hash(),
            "test",
            "1",
            "甲方：华邦文化传媒有限公司\n乙方：示例客户有限公司\n合同金额：人民币10000元\n付款安排：签约后支付50%，验收通过后支付尾款\n验收标准：按双方确认的成片脚本与技术要求验收。",
            10,
        );
        let output = ContractReviewRuleEngine.evaluate("review-1", &extraction, 11);
        assert!(output.findings.is_empty());
        assert_eq!(output.evaluations.len(), 4);
        assert!(output
            .evaluations
            .iter()
            .all(|value| value.status == RuleEvaluationStatus::Passed));
        assert!(!output.evidence.is_empty());
    }

    #[test]
    fn missing_contract_basics_produce_structured_missing_findings() {
        let extraction = build_extraction_from_text(
            "review-1",
            "asset-1",
            &hash(),
            "test",
            "1",
            "本文件仅用于说明合作意向，具体商务条件另行协商。",
            10,
        );
        let output = ContractReviewRuleEngine.evaluate("review-1", &extraction, 11);
        assert_eq!(output.findings.len(), 4);
        assert!(output.findings.iter().all(|finding| {
            finding.evidence_ids.is_empty() && finding.missing_evidence_reason.is_some()
        }));
        assert!(output
            .evaluations
            .iter()
            .all(|value| value.status == RuleEvaluationStatus::Finding));
    }
}
