---
name: business-document-pipeline
description: 处理 PDF、DOCX、XLSX、图片和纯文本商务资料，完成识别、提取、OCR/表格降级、来源定位、字段归一、模板生成、渲染检查和最终 Artifact 交付。
---

# 商务文档生产管线

## 触发条件
- 收到合同、报价单、招标文件、请款、验收或客户附件
- 需要读取或生成 PDF、DOCX、XLSX、图片或纯文本商务文件
- 文档存在扫描页、跨页表格、模板格式或输出质量要求

## 输入
- 一个或多个原始文档 Artifact
- 可选的项目、客户、文档类型、公司模板和预期用途

## 执行步骤
1. 识别文件类型、版本、页数/工作表、可提取性和完整性，不改变原件。
2. 根据内容选择文本、表格或 OCR 路由；记录 parser/OCR provenance、页码和置信度。
3. 提取并归一客户、项目、金额、税率、日期、联系人、交付、付款和验收字段。
4. 将字段标记为已确认、待确认、冲突、缺失或无法判断，并输出来源定位。
5. 需要生成文件时套用已批准模板，保留格式、公式、分页、页眉页脚、签章位和附件编号。
6. 重新打开并验证生成文件，检查漏页、表格截断、公式错误、金额异常和不可读内容。
7. 修复后重新验证，最终结果写入 Vault Artifact；聊天只返回短预览和稳定 assetId。

## 人工确认边界
- OCR、表格识别、模板映射和关键字段在进入正式主档前必须确认。
- 正式对外 DOCX/PDF/XLSX、签章位置、金额和附件清单必须人工确认。

## 输出 Artifact
- `business.document.intake-summary`
- `business.document.structured-extraction`
- `business.document.generated-draft`
- `business.document.qa-report`

## 禁止
- 不覆盖原始文件，不把预览文本当作唯一权威。
- 不复制 Proma 的专有文档脚本、Prompt 或模板。
- 不把不同版本、不同项目或不同客户的字段静默合并。

## 所需工具与权限

工具：
- `business.document.extract`
- `business.document.generate`
- `business.document.validate`
- `business.source.locate`
- `business.template.read`
- `business.artifact.create`
- `business.approval.request`

权限：
- `document.read:business`
- `document.generate:draft`
- `artifact.write:draft`
