---
name: contract-review
description: 审查商务合同并与 Brief、报价和项目主档核对，输出摘要、风险、缺失条款、待确认问题和建议修改文本。用于合同初审、修订版复核、签署前检查。
---

# 合同审查

## 触发条件
- 收到客户合同或公司合同模板
- 合同修订版需要复核
- 签署前需要一致性检查
- 付款、验收、版权或违约条款不清

## 输入
- 合同原件或可提取文本
- 已确认 Brief
- 最终报价或报价草案
- 项目主档及公司条款参考

## 执行步骤
1. 提取主体、金额、税率、付款节点、交付物、期限、验收、知识产权、保密、违约、终止和争议解决。
2. 逐项关联原文页码、段落或定位信息；无法定位时明确标记。
3. 与 Brief、报价、项目主档做一致性比对。
4. 按高/中/低列风险、影响、依据和建议动作。
5. 生成缺失条款、待确认问题及可编辑修改建议；保留不确定性。

## 人工确认边界
- AI 只做业务初审，不作法律结论。
- 签署、盖章、接受高风险条款或对外发送修改意见必须由授权人员确认。

## 输出 Artifact
- `business.contract.summary`
- `business.contract.risk-report`
- `business.contract.questions`
- `business.contract.redline-suggestions`

## 禁止
- 不得声称合同“绝对安全”或“无法律风险”。
- 不得臆造法律依据、原文条款、公司政策或对方意图。

## 所需工具与权限

工具：
- `business.document.extract`
- `business.source.locate`
- `business.project.read`
- `business.artifact.compare`
- `business.artifact.create`
- `business.approval.request`

权限：
- `document.read:contract`
- `project.read`
- `artifact.read`
- `artifact.write:draft`
