use crate::contract_review_rules::RULE_SET_VERSION;
use crate::protocol::{ContractReviewRecord, HostError, ReviewFindingDecision, ReviewReportFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const MAX_REPORT_ID_LENGTH: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedReviewReport {
    pub report_id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub format: ReviewReportFormat,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewReportPayload<'a> {
    report_id: &'a str,
    review_id: &'a str,
    review_revision: i64,
    source_asset_id: &'a str,
    source_asset_sha256: &'a str,
    rule_set_version: &'static str,
    summary: ReportSummary,
    contract_review: &'a ContractReviewRecord,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportSummary {
    total_findings: usize,
    confirmed: usize,
    rejected: usize,
    accepted_risk: usize,
    needs_revision: usize,
    unreviewed: usize,
}

pub fn generate_review_report(
    review: &ContractReviewRecord,
    format: ReviewReportFormat,
    staging_root: &Path,
) -> Result<GeneratedReviewReport, HostError> {
    let report_id = Uuid::new_v4().to_string();
    generate_review_report_with_id(review, format, staging_root, &report_id)
}

pub fn generate_review_report_with_id(
    review: &ContractReviewRecord,
    format: ReviewReportFormat,
    staging_root: &Path,
    report_id: &str,
) -> Result<GeneratedReviewReport, HostError> {
    validate_report_id(report_id)?;
    validate_review_for_report(review)?;
    fs::create_dir_all(staging_root).map_err(|error| {
        HostError::new(
            "REVIEW_REPORT_IO_FAILED",
            format!("unable to prepare review report staging directory: {error}"),
            true,
        )
    })?;
    let extension = match format {
        ReviewReportFormat::Json => "json",
        ReviewReportFormat::Html => "html",
        ReviewReportFormat::Docx => "docx",
    };
    let path = staging_root.join(format!("review-report-{report_id}.{extension}"));
    let payload = ReviewReportPayload {
        report_id,
        review_id: &review.session.id,
        review_revision: review.session.revision,
        source_asset_id: &review.session.source_asset_id,
        source_asset_sha256: &review.session.source_asset_sha256,
        rule_set_version: RULE_SET_VERSION,
        summary: summarize(review),
        contract_review: review,
    };
    let bytes = match format {
        ReviewReportFormat::Json => serde_json::to_vec_pretty(&payload).map_err(|error| {
            HostError::new(
                "REVIEW_REPORT_SERIALIZATION_FAILED",
                format!("unable to serialize review report: {error}"),
                false,
            )
        })?,
        ReviewReportFormat::Html => render_html(&payload).into_bytes(),
        ReviewReportFormat::Docx => render_docx(&payload)?,
    };
    write_atomic(&path, &bytes)?;
    Ok(GeneratedReviewReport {
        report_id: report_id.to_string(),
        path,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        format,
    })
}

fn validate_report_id(report_id: &str) -> Result<(), HostError> {
    if report_id.trim().is_empty() || report_id != report_id.trim() {
        return Err(HostError::validation(
            "reportId must be non-empty and trimmed",
        ));
    }
    if report_id.chars().count() > MAX_REPORT_ID_LENGTH {
        return Err(HostError::validation(format!(
            "reportId exceeds {MAX_REPORT_ID_LENGTH} characters"
        )));
    }
    if !report_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(HostError::validation(
            "reportId must contain only ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(())
}

fn validate_review_for_report(review: &ContractReviewRecord) -> Result<(), HostError> {
    if review.extraction.is_none() {
        return Err(HostError::validation(
            "contract review extraction must exist before report generation",
        ));
    }
    if review
        .findings
        .iter()
        .any(|finding| finding.decision == ReviewFindingDecision::Unreviewed)
    {
        return Err(HostError::validation(
            "all review findings require a human decision before report generation",
        ));
    }
    Ok(())
}

fn summarize(review: &ContractReviewRecord) -> ReportSummary {
    let mut summary = ReportSummary {
        total_findings: review.findings.len(),
        confirmed: 0,
        rejected: 0,
        accepted_risk: 0,
        needs_revision: 0,
        unreviewed: 0,
    };
    for finding in &review.findings {
        match finding.decision {
            ReviewFindingDecision::Confirmed => summary.confirmed += 1,
            ReviewFindingDecision::Rejected => summary.rejected += 1,
            ReviewFindingDecision::AcceptedRisk => summary.accepted_risk += 1,
            ReviewFindingDecision::NeedsRevision => summary.needs_revision += 1,
            ReviewFindingDecision::Unreviewed => summary.unreviewed += 1,
        }
    }
    summary
}

fn render_html(payload: &ReviewReportPayload<'_>) -> String {
    let review = payload.contract_review;
    let mut findings = String::new();
    for finding in &review.findings {
        findings.push_str("<article class=\"finding\">");
        findings.push_str(&format!(
            "<h3>{}</h3><p><b>严重级别：</b>{:?} · <b>人工结论：</b>{:?}</p>",
            escape_html(&finding.title),
            finding.severity,
            finding.decision
        ));
        findings.push_str(&format!(
            "<p>{}</p><p><b>处理建议：</b>{}</p>",
            escape_html(&finding.description),
            escape_html(&finding.recommendation)
        ));
        if let Some(reason) = &finding.missing_evidence_reason {
            findings.push_str(&format!(
                "<p><b>缺失证据说明：</b>{}</p>",
                escape_html(reason)
            ));
        }
        findings.push_str("</article>");
    }
    format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>合同审查报告</title><style>body{{font-family:system-ui,'Microsoft YaHei',sans-serif;max-width:960px;margin:40px auto;padding:0 24px;color:#202124}}header,.finding{{border:1px solid #ddd;border-radius:12px;padding:18px;margin:14px 0}}h1,h3{{margin-top:0}}code{{word-break:break-all}}</style></head><body><header><h1>合同审查报告</h1><p>Report ID: <code>{}</code></p><p>审查编号：{}</p><p>本地源资产：<code>{}</code></p><p>源文件 SHA-256：<code>{}</code></p><p>规则版本：{}</p><p>风险总数：{} · 已确认：{} · 已驳回：{} · 接受风险：{} · 需要修改：{}</p></header>{}</body></html>",
        escape_html(payload.report_id),
        escape_html(payload.review_id),
        escape_html(payload.source_asset_id),
        escape_html(payload.source_asset_sha256),
        escape_html(payload.rule_set_version),
        payload.summary.total_findings,
        payload.summary.confirmed,
        payload.summary.rejected,
        payload.summary.accepted_risk,
        payload.summary.needs_revision,
        findings
    )
}

fn render_docx(payload: &ReviewReportPayload<'_>) -> Result<Vec<u8>, HostError> {
    let document_xml = render_docx_document(payload);
    let core_properties = render_docx_core_properties(payload);
    let parts = [
        ("[Content_Types].xml", DOCX_CONTENT_TYPES),
        ("_rels/.rels", DOCX_ROOT_RELATIONSHIPS),
        ("docProps/core.xml", core_properties.as_str()),
        ("docProps/app.xml", DOCX_APP_PROPERTIES),
        ("word/document.xml", document_xml.as_str()),
        ("word/styles.xml", DOCX_STYLES),
        ("word/_rels/document.xml.rels", DOCX_DOCUMENT_RELATIONSHIPS),
    ];
    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (name, contents) in parts {
        archive
            .start_file(name, options)
            .map_err(|error| docx_package_error("create OOXML part", error))?;
        archive
            .write_all(contents.as_bytes())
            .map_err(|error| docx_package_error("write OOXML part", error))?;
    }
    let cursor = archive
        .finish()
        .map_err(|error| docx_package_error("finish OOXML package", error))?;
    Ok(cursor.into_inner())
}

fn render_docx_document(payload: &ReviewReportPayload<'_>) -> String {
    let review = payload.contract_review;
    let extraction = review
        .extraction
        .as_ref()
        .expect("validated contract review has an extraction");
    let mut body = String::new();

    push_docx_paragraph(&mut body, "合同审查报告", Some("Title"), true);
    push_docx_paragraph(&mut body, "报告信息", Some("Heading1"), true);
    for line in [
        format!("Report ID：{}", payload.report_id),
        format!("审查编号：{}", payload.review_id),
        format!("审查版本：{}", payload.review_revision),
        format!("合同文件：{}", review.session.source_file_name),
        format!("本地源资产：{}", payload.source_asset_id),
        format!("源文件 SHA-256：{}", payload.source_asset_sha256),
        format!("解析快照 ID：{}", extraction.id),
        format!(
            "解析内容 SHA-256：{}",
            extraction.content_sha256.as_deref().unwrap_or("未生成")
        ),
        format!(
            "解析器：{} {}（{}）",
            extraction.parser.name, extraction.parser.version, extraction.parser.mode
        ),
        format!("规则版本：{}", payload.rule_set_version),
        format!(
            "审查状态：{:?} / {:?}",
            review.session.status, review.session.stage
        ),
    ] {
        push_docx_paragraph(&mut body, &line, None, false);
    }

    push_docx_paragraph(&mut body, "风险摘要", Some("Heading1"), true);
    for line in [
        format!("风险总数：{}", payload.summary.total_findings),
        format!("已确认：{}", payload.summary.confirmed),
        format!("已驳回：{}", payload.summary.rejected),
        format!("接受风险：{}", payload.summary.accepted_risk),
        format!("需要修改：{}", payload.summary.needs_revision),
        format!("未处理：{}", payload.summary.unreviewed),
    ] {
        push_docx_paragraph(&mut body, &line, None, false);
    }

    push_docx_paragraph(&mut body, "Findings", Some("Heading1"), true);
    if review.findings.is_empty() {
        push_docx_paragraph(&mut body, "未发现风险项。", None, false);
    }
    for (index, finding) in review.findings.iter().enumerate() {
        push_docx_paragraph(
            &mut body,
            &format!("风险 {}：{}", index + 1, finding.title),
            Some("Heading2"),
            true,
        );
        for line in [
            format!("Finding ID：{}", finding.id),
            format!("来源：{:?}", finding.source),
            format!("分类：{}", finding.category),
            format!("严重级别：{:?}", finding.severity),
            format!("状态：{:?}", finding.status),
            format!("人工结论：{:?}", finding.decision),
            format!("问题说明：{}", finding.description),
            format!("处理建议：{}", finding.recommendation),
            format!(
                "规则：{} / {}",
                finding.rule_id.as_deref().unwrap_or("—"),
                finding.rule_version.as_deref().unwrap_or("—")
            ),
            format!(
                "Agent Run ID：{}",
                finding.agent_run_id.as_deref().unwrap_or("—")
            ),
            format!(
                "关联 Evidence：{}",
                if finding.evidence_ids.is_empty() {
                    "—".to_string()
                } else {
                    finding.evidence_ids.join("、")
                }
            ),
        ] {
            push_docx_paragraph(&mut body, &line, None, false);
        }
        if let Some(reason) = &finding.missing_evidence_reason {
            push_docx_paragraph(&mut body, &format!("缺失证据说明：{reason}"), None, false);
        }

        let decisions = review
            .decisions
            .iter()
            .filter(|decision| decision.finding_id == finding.id)
            .collect::<Vec<_>>();
        if decisions.is_empty() {
            push_docx_paragraph(&mut body, "人工决策记录：无", None, false);
        } else {
            push_docx_paragraph(&mut body, "人工决策记录", Some("Heading2"), true);
            for decision in decisions {
                for line in [
                    format!("Decision ID：{}", decision.id),
                    format!("结论：{:?}", decision.decision),
                    format!("处理人：{}", decision.actor_id),
                    format!("意见：{}", decision.comment),
                    format!("Finding Revision：{}", decision.finding_revision),
                    format!("记录时间：{}", decision.created_at),
                ] {
                    push_docx_paragraph(&mut body, &line, None, false);
                }
            }
        }
    }

    push_docx_paragraph(&mut body, "Evidence", Some("Heading1"), true);
    if review.evidence.is_empty() {
        push_docx_paragraph(&mut body, "无可定位证据。", None, false);
    }
    for (index, evidence) in review.evidence.iter().enumerate() {
        push_docx_paragraph(
            &mut body,
            &format!("证据 {}", index + 1),
            Some("Heading2"),
            true,
        );
        let char_range = match (evidence.char_start, evidence.char_end) {
            (Some(start), Some(end)) => format!("{start}..{end}"),
            _ => "—".to_string(),
        };
        for line in [
            format!("Evidence ID：{}", evidence.id),
            format!("页码：{}", evidence.page_index + 1),
            format!("Block ID：{}", evidence.block_id.as_deref().unwrap_or("—")),
            format!("字符范围：{char_range}"),
            format!("引用原文：{}", evidence.quoted_text),
            format!("引用 SHA-256：{}", evidence.quoted_text_sha256),
            format!("上文：{}", evidence.context_before),
            format!("下文：{}", evidence.context_after),
            format!("来源资产：{}", evidence.source_asset_id),
        ] {
            push_docx_paragraph(&mut body, &line, None, false);
        }
    }

    push_docx_paragraph(&mut body, "规则执行记录", Some("Heading1"), true);
    if review.rule_evaluations.is_empty() {
        push_docx_paragraph(&mut body, "无规则执行记录。", None, false);
    }
    for evaluation in &review.rule_evaluations {
        push_docx_paragraph(
            &mut body,
            &format!("{} / {}", evaluation.rule_id, evaluation.rule_version),
            Some("Heading2"),
            true,
        );
        for line in [
            format!("状态：{:?}", evaluation.status),
            format!("说明：{}", evaluation.details),
            format!(
                "关联 Findings：{}",
                if evaluation.finding_ids.is_empty() {
                    "—".to_string()
                } else {
                    evaluation.finding_ids.join("、")
                }
            ),
            format!("执行时间：{}", evaluation.evaluated_at),
        ] {
            push_docx_paragraph(&mut body, &line, None, false);
        }
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>"
    )
}

fn push_docx_paragraph(body: &mut String, text: &str, style: Option<&str>, bold: bool) {
    body.push_str("<w:p>");
    if let Some(style) = style {
        body.push_str("<w:pPr><w:pStyle w:val=\"");
        body.push_str(&escape_xml(style));
        body.push_str("\"/></w:pPr>");
    }
    body.push_str("<w:r>");
    if bold {
        body.push_str("<w:rPr><w:b/></w:rPr>");
    }
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            body.push_str("<w:br/>");
        }
        body.push_str("<w:t xml:space=\"preserve\">");
        body.push_str(&escape_xml(line));
        body.push_str("</w:t>");
    }
    body.push_str("</w:r></w:p>");
}

fn render_docx_core_properties(payload: &ReviewReportPayload<'_>) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>合同审查报告</dc:title><dc:subject>{}</dc:subject><dc:creator>华邦互娱商务系统</dc:creator><cp:keywords>合同审查,{},SHA-256</cp:keywords><dc:description>基于本地合同资产生成的审查报告，来源 SHA-256：{}</dc:description><cp:lastModifiedBy>华邦互娱商务系统</cp:lastModifiedBy><dcterms:created xsi:type=\"dcterms:W3CDTF\">2000-01-01T00:00:00Z</dcterms:created><dcterms:modified xsi:type=\"dcterms:W3CDTF\">2000-01-01T00:00:00Z</dcterms:modified></cp:coreProperties>",
        escape_xml(payload.report_id),
        escape_xml(payload.review_id),
        escape_xml(payload.source_asset_sha256)
    )
}

fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' => {
                escaped.push(character)
            }
            _ => {}
        }
    }
    escaped
}

fn docx_package_error(action: &str, error: impl std::fmt::Display) -> HostError {
    HostError::new(
        "REVIEW_REPORT_SERIALIZATION_FAILED",
        format!("unable to {action}: {error}"),
        false,
    )
}

const DOCX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#;

const DOCX_ROOT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

const DOCX_DOCUMENT_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#;

const DOCX_APP_PROPERTIES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>华邦互娱商务系统</Application>
  <AppVersion>1.0</AppVersion>
  <Company>华邦互娱</Company>
</Properties>"#;

const DOCX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault><w:rPr><w:rFonts w:ascii="Microsoft YaHei" w:hAnsi="Microsoft YaHei" w:eastAsia="Microsoft YaHei"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault>
    <w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="360" w:lineRule="auto"/></w:pPr></w:pPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
  <w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/><w:spacing w:before="240" w:after="360"/></w:pPr><w:rPr><w:b/><w:sz w:val="36"/><w:szCs w:val="36"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="320" w:after="160"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr></w:style>
</w:styles>"#;

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), HostError> {
    match fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Ok(_) => {
            return Err(HostError::new(
                "REVIEW_REPORT_ID_CONFLICT",
                "a different review report already exists for this reportId and format",
                false,
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(HostError::new(
                "REVIEW_REPORT_IO_FAILED",
                format!("unable to inspect existing review report: {error}"),
                true,
            ))
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| HostError::internal("review report path has no parent"))?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = File::create(&temporary).map_err(|error| {
            HostError::new(
                "REVIEW_REPORT_IO_FAILED",
                format!("unable to create review report staging file: {error}"),
                true,
            )
        })?;
        file.write_all(bytes).map_err(|error| {
            HostError::new(
                "REVIEW_REPORT_IO_FAILED",
                format!("unable to write review report: {error}"),
                true,
            )
        })?;
        file.flush().map_err(|error| {
            HostError::new(
                "REVIEW_REPORT_IO_FAILED",
                format!("unable to flush review report: {error}"),
                true,
            )
        })?;
        file.sync_all().map_err(|error| {
            HostError::new(
                "REVIEW_REPORT_IO_FAILED",
                format!("unable to sync review report: {error}"),
                true,
            )
        })?;
        fs::rename(&temporary, path).map_err(|error| {
            HostError::new(
                "REVIEW_REPORT_IO_FAILED",
                format!("unable to commit review report: {error}"),
                true,
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_intelligence::build_extraction_from_text;
    use crate::protocol::{ContractReviewSessionRecord, ContractReviewStage, ContractReviewStatus};
    use std::io::Read;
    use tempfile::tempdir;

    fn review() -> ContractReviewRecord {
        let extraction = build_extraction_from_text(
            "review-1",
            "asset-1",
            &"a".repeat(64),
            "test",
            "1",
            "甲方与乙方确认合同金额为100元，付款后按约定标准验收。",
            10,
        );
        ContractReviewRecord {
            session: ContractReviewSessionRecord {
                id: "review-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                source_asset_id: "asset-1".to_string(),
                source_asset_sha256: "a".repeat(64),
                source_file_name: "contract.pdf".to_string(),
                status: ContractReviewStatus::AwaitingConfirmation,
                stage: ContractReviewStage::AwaitingConfirmation,
                extraction_id: Some(extraction.id.clone()),
                report_asset_id: None,
                revision: 2,
                created_at: 10,
                updated_at: 10,
                completed_at: None,
                failure: None,
            },
            extraction: Some(extraction),
            evidence: Vec::new(),
            findings: Vec::new(),
            rule_evaluations: Vec::new(),
            decisions: Vec::new(),
            reports: Vec::new(),
        }
    }

    #[test]
    fn stable_report_id_makes_same_review_retry_deterministic() {
        let temp = tempdir().unwrap();
        let review = review();
        let report_id = "report-review-1-revision-2";

        let first = generate_review_report_with_id(
            &review,
            ReviewReportFormat::Json,
            temp.path(),
            report_id,
        )
        .unwrap();
        let first_bytes = fs::read(&first.path).unwrap();
        let second = generate_review_report_with_id(
            &review,
            ReviewReportFormat::Json,
            temp.path(),
            report_id,
        )
        .unwrap();
        let second_bytes = fs::read(&second.path).unwrap();

        assert_eq!(first.report_id, report_id);
        assert_eq!(second.report_id, report_id);
        assert_eq!(first.path, second.path);
        assert_eq!(
            first.path.file_name().unwrap(),
            "review-report-report-review-1-revision-2.json"
        );
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first_bytes, second_bytes);
        let payload: serde_json::Value = serde_json::from_slice(&first_bytes).unwrap();
        assert_eq!(payload["reportId"], report_id);
    }

    #[test]
    fn json_and_html_reports_use_stable_id_and_keep_format_specific_paths() {
        let temp = tempdir().unwrap();
        let review = review();
        let report_id = "report-review-1-revision-2";

        for (format, extension) in [
            (ReviewReportFormat::Json, "json"),
            (ReviewReportFormat::Html, "html"),
        ] {
            let generated =
                generate_review_report_with_id(&review, format, temp.path(), report_id).unwrap();
            assert!(generated.path.is_file());
            assert_eq!(generated.report_id, report_id);
            assert_eq!(generated.format, format);
            assert_eq!(generated.path.extension().unwrap(), extension);
            assert_eq!(
                generated.path.file_name().unwrap().to_str().unwrap(),
                format!("review-report-{report_id}.{extension}")
            );
            assert_eq!(generated.sha256.len(), 64);
            let content = String::from_utf8(fs::read(&generated.path).unwrap()).unwrap();
            assert!(content.contains(report_id));
        }
    }

    #[test]
    fn report_id_must_be_non_empty_and_safe_for_filenames() {
        let temp = tempdir().unwrap();
        for invalid_report_id in [
            "",
            " ",
            " report",
            "report ",
            "../report",
            "report/name",
            "report\\name",
            "report.id",
        ] {
            let error = generate_review_report_with_id(
                &review(),
                ReviewReportFormat::Json,
                temp.path(),
                invalid_report_id,
            )
            .unwrap_err();
            assert_eq!(error.code, "VALIDATION_FAILED");
        }

        let too_long = "r".repeat(MAX_REPORT_ID_LENGTH + 1);
        let error = generate_review_report_with_id(
            &review(),
            ReviewReportFormat::Json,
            temp.path(),
            &too_long,
        )
        .unwrap_err();
        assert_eq!(error.code, "VALIDATION_FAILED");
    }

    #[test]
    fn docx_report_is_deterministic_valid_ooxml_with_audit_content() {
        let temp = tempdir().unwrap();
        let report_id = "report-review-1-revision-2";
        let first = generate_review_report_with_id(
            &review(),
            ReviewReportFormat::Docx,
            temp.path(),
            report_id,
        )
        .unwrap();
        let first_bytes = fs::read(&first.path).unwrap();
        let second = generate_review_report_with_id(
            &review(),
            ReviewReportFormat::Docx,
            temp.path(),
            report_id,
        )
        .unwrap();
        assert_eq!(first.report_id, report_id);
        assert_eq!(first.path, second.path);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first_bytes, fs::read(&second.path).unwrap());

        let mut archive = zip::ZipArchive::new(Cursor::new(first_bytes)).unwrap();
        for required in [
            "[Content_Types].xml",
            "_rels/.rels",
            "docProps/core.xml",
            "word/document.xml",
            "word/styles.xml",
            "word/_rels/document.xml.rels",
        ] {
            assert!(archive.by_name(required).is_ok(), "missing {required}");
        }
        let mut document_xml = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut document_xml)
            .unwrap();
        assert!(document_xml.contains("合同审查报告"));
        assert!(document_xml.contains(report_id));
        assert!(document_xml.contains("asset-1"));
        assert!(document_xml.contains(&"a".repeat(64)));
    }
}
