---
name: business-review-board
description: 对高风险合同、报价、标书、请款、验收和对外材料做独立的商务、财务、交付、风险与一致性复核，输出分歧和最终人工审批清单。
---

# 商务独立复核

## 触发条件
- 大金额、年框、客户版或复杂付款合同
- 标书、报价、请款、验收或合同即将对外发送
- 主审结果需要独立寻找遗漏、矛盾和错误假设

## 输入
- 原始材料和待复核 Artifact
- 当前项目主档、报价、合同、请款、验收和 Ledger 状态
- 风险阈值和本次复核目标

## 执行步骤
1. 先冻结待复核 revision；复核者不能直接修改主结果。
2. 分别从商务、财务、交付、风险和跨文档一致性角度独立检查。
3. 每项结论必须关联原文、Artifact revision 或计算口径，区分确认问题和合理分歧。
4. 对客户主体、金额、税率、付款、交付、验收、知识产权、违约、版本、附件、签章和日期执行对外发送前检查。
5. 主 Agent 汇总为采纳、驳回、待补证和需人工裁决，不静默合并分歧。
6. 人工确认后生成决策包；原始复核意见和处理结果都保留。

## 人工确认边界
- 复核角色只提出问题，不直接签署、发送、核销、验收或接受风险。
- 高风险差异、审计豁免和所有对外文件必须由授权人员裁决。

## 输出 Artifact
- `business.review.board-findings`
- `business.review.disagreements`
- `business.review.outbound-checklist`
- `business.review.decision-pack`

## 禁止
- 不让同一上下文中的一次自我复述冒充独立复核。
- 不引入 Proma collaboration Runtime 或第二套多 Agent 权威。
- 不因复核者意见而直接覆盖主 Artifact。

## 所需工具与权限

工具：
- `business.project.read`
- `business.ledger.read`
- `business.artifact.read`
- `business.artifact.compare`
- `business.source.locate`
- `business.artifact.create`
- `business.approval.request`

权限：
- `project.read`
- `ledger.read`
- `artifact.read`
- `artifact.write:draft`
